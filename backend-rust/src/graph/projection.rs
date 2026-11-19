//! Idempotent Graph Projection Engine.
//!
//! Transforms committed relational outbox records into deterministic,
//! parameterized Cypher statements using `MERGE` semantics.
//! Preserves provenance, case scoping, and technical identifier privacy.

use std::sync::Arc;
use tracing::{debug, info};

use super::client::GraphClient;
use super::model::{CypherStatement, OutboxEvent};
use super::privacy::compute_identifier_fingerprint;
use crate::error::AppError;

pub struct GraphProjectionService {
    client: Arc<dyn GraphClient>,
    hmac_key: Option<String>,
}

impl GraphProjectionService {
    pub fn new(client: Arc<dyn GraphClient>, hmac_key: Option<String>) -> Self {
        Self { client, hmac_key }
    }

    /// Projects a single outbox event into Neo4j.
    pub async fn project_event(&self, event: &OutboxEvent) -> Result<(), AppError> {
        let statements = self.generate_cypher_statements(event)?;
        debug!(
            event_type = %event.event_type,
            statement_count = statements.len(),
            "Executing graph projection Cypher batch"
        );
        self.client.execute_batch(statements).await?;
        info!(
            outbox_id = %event.outbox_id,
            aggregate_id = %event.aggregate_id,
            event_type = %event.event_type,
            "Successfully projected outbox event to Neo4j"
        );
        Ok(())
    }

    /// Generates idempotent Cypher statements for an outbox event.
    pub fn generate_cypher_statements(
        &self,
        event: &OutboxEvent,
    ) -> Result<Vec<CypherStatement>, AppError> {
        let mut statements = Vec::new();
        let p = &event.payload;

        match event.event_type.as_str() {
            "CASE_CREATED" => {
                let case_id = event.case_id.to_string();
                let title = p
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled Case");
                let status = p.get("status").and_then(|v| v.as_str()).unwrap_or("DRAFT");
                let created_at = p.get("created_at").and_then(|v| v.as_str()).unwrap_or("");

                statements.push(CypherStatement::new(
                    r#"
                    MERGE (c:Case {case_id: $case_id})
                    ON CREATE SET c.title = $title, c.status = $status, c.created_at = $created_at
                    ON MATCH SET c.title = $title, c.status = $status
                    "#,
                    serde_json::json!({
                        "case_id": case_id,
                        "title": title,
                        "status": status,
                        "created_at": created_at,
                    }),
                ));
            }

            "EVIDENCE_INGESTED" => {
                let case_id = event.case_id.to_string();
                let evidence_id = event.aggregate_id.to_string();
                let sha256_hash = p.get("sha256_hash").and_then(|v| v.as_str()).unwrap_or("");
                let source_type = p.get("source_type").and_then(|v| v.as_str()).unwrap_or("");
                let mime_type = p.get("mime_type").and_then(|v| v.as_str()).unwrap_or("");
                let file_size_bytes = p
                    .get("file_size_bytes")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let storage_uri = p.get("storage_uri").and_then(|v| v.as_str()).unwrap_or("");
                let created_at = p.get("created_at").and_then(|v| v.as_str()).unwrap_or("");

                statements.push(CypherStatement::new(
                    r#"
                    MERGE (c:Case {case_id: $case_id})
                    MERGE (e:Evidence {evidence_id: $evidence_id})
                    ON CREATE SET e.case_id = $case_id, e.sha256_hash = $sha256_hash,
                                  e.source_type = $source_type, e.mime_type = $mime_type,
                                  e.file_size_bytes = $file_size_bytes, e.storage_uri = $storage_uri,
                                  e.created_at = $created_at
                    MERGE (c)-[:HAS_EVIDENCE]->(e)
                    "#,
                    serde_json::json!({
                        "case_id": case_id,
                        "evidence_id": evidence_id,
                        "sha256_hash": sha256_hash,
                        "source_type": source_type,
                        "mime_type": mime_type,
                        "file_size_bytes": file_size_bytes,
                        "storage_uri": storage_uri,
                        "created_at": created_at,
                    }),
                ));
            }

            "EXTRACTION_COMPLETED" => {
                let case_id = event.case_id.to_string();
                let run_id = event.aggregate_id.to_string();
                let evidence_id = p
                    .get("evidence_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let extractor_version = p
                    .get("extractor_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1.0.0");
                let config_hash = p
                    .get("configuration_hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let result_sha256 = p
                    .get("result_sha256")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let status = p
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("SUCCESS");
                let duration_ms = p.get("duration_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);

                // 1. Extraction run node
                statements.push(CypherStatement::new(
                    r#"
                    MERGE (e:Evidence {evidence_id: $evidence_id})
                    MERGE (r:ExtractionRun {extraction_run_id: $run_id})
                    ON CREATE SET r.case_id = $case_id, r.evidence_id = $evidence_id,
                                  r.extractor_version = $extractor_version, r.configuration_hash = $config_hash,
                                  r.result_sha256 = $result_sha256, r.status = $status, r.duration_ms = $duration_ms
                    MERGE (e)-[:PRODUCED_EXTRACTION_RUN]->(r)
                    "#,
                    serde_json::json!({
                        "case_id": case_id,
                        "evidence_id": evidence_id,
                        "run_id": run_id,
                        "extractor_version": extractor_version,
                        "config_hash": config_hash,
                        "result_sha256": result_sha256,
                        "status": status,
                        "duration_ms": duration_ms,
                    }),
                ));

                // 2. Entities with privacy-preserving HMAC fingerprints
                if let Some(entities) = p.get("entities").and_then(|v| v.as_array()) {
                    for ent in entities {
                        let entity_type = ent
                            .get("identifier_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("UNKNOWN");
                        let canonical_val = ent
                            .get("canonical_identifier")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let confidence = ent
                            .get("confidence")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(1.0);

                        let fingerprint = compute_identifier_fingerprint(
                            &event.case_id,
                            entity_type,
                            canonical_val,
                            self.hmac_key.as_deref(),
                        );
                        let graph_entity_id = format!("{}:{}", case_id, fingerprint);

                        statements.push(CypherStatement::new(
                            r#"
                            MERGE (r:ExtractionRun {extraction_run_id: $run_id})
                            MERGE (ent:EntityCandidate {graph_entity_id: $graph_entity_id})
                            ON CREATE SET ent.case_id = $case_id, ent.extraction_run_id = $run_id,
                                          ent.entity_type = $entity_type, ent.fingerprint = $fingerprint,
                                          ent.confidence = $confidence
                            MERGE (r)-[:EXTRACTED_ENTITY]->(ent)
                            "#,
                            serde_json::json!({
                                "run_id": run_id,
                                "graph_entity_id": graph_entity_id,
                                "case_id": case_id,
                                "entity_type": entity_type,
                                "fingerprint": fingerprint,
                                "confidence": confidence,
                            }),
                        ));
                    }
                }

                // 3. Events
                if let Some(events) = p.get("events").and_then(|v| v.as_array()) {
                    for ev in events {
                        let event_id = ev.get("event_id").and_then(|v| v.as_str()).unwrap_or("");
                        let source_rec = ev
                            .get("source_record_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let ev_type = ev
                            .get("event_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("UNKNOWN");
                        let timestamp = ev
                            .get("raw_timestamp")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let duration = ev.get("duration_seconds").and_then(|v| v.as_f64());
                        let lat = ev.get("latitude").and_then(|v| v.as_f64());
                        let lon = ev.get("longitude").and_then(|v| v.as_f64());

                        statements.push(CypherStatement::new(
                            r#"
                            MERGE (r:ExtractionRun {extraction_run_id: $run_id})
                            MERGE (ev:EventCandidate {event_id: $event_id})
                            ON CREATE SET ev.case_id = $case_id, ev.evidence_id = $evidence_id,
                                          ev.extraction_run_id = $run_id, ev.source_record_id = $source_rec,
                                          ev.event_type = $ev_type, ev.nominal_timestamp = $timestamp,
                                          ev.duration_seconds = $duration, ev.latitude = $lat, ev.longitude = $lon
                            MERGE (r)-[:EXTRACTED_EVENT]->(ev)
                            "#,
                            serde_json::json!({
                                "run_id": run_id,
                                "event_id": event_id,
                                "case_id": case_id,
                                "evidence_id": evidence_id,
                                "source_rec": source_rec,
                                "ev_type": ev_type,
                                "timestamp": timestamp,
                                "duration": duration,
                                "lat": lat,
                                "lon": lon,
                            }),
                        ));
                    }
                }

                // 4. Relationship assertions
                if let Some(relationships) = p.get("relationships").and_then(|v| v.as_array()) {
                    for (idx, rel) in relationships.iter().enumerate() {
                        let assertion_id = format!("{}:{}", run_id, idx);
                        let rel_type = rel
                            .get("relation_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("COMMUNICATED_WITH");
                        let src_raw = rel
                            .get("source_raw_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let tgt_raw = rel
                            .get("target_raw_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let is_observed = rel
                            .get("is_observed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);

                        let src_fp = compute_identifier_fingerprint(
                            &event.case_id,
                            "PHONE",
                            src_raw,
                            self.hmac_key.as_deref(),
                        );
                        let tgt_fp = compute_identifier_fingerprint(
                            &event.case_id,
                            "PHONE",
                            tgt_raw,
                            self.hmac_key.as_deref(),
                        );
                        let src_gid = format!("{}:{}", case_id, src_fp);
                        let tgt_gid = format!("{}:{}", case_id, tgt_fp);

                        statements.push(CypherStatement::new(
                            r#"
                            MERGE (r:ExtractionRun {extraction_run_id: $run_id})
                            MERGE (e:Evidence {evidence_id: $evidence_id})
                            MERGE (rel:RelationshipAssertion {assertion_id: $assertion_id})
                            ON CREATE SET rel.case_id = $case_id, rel.extraction_run_id = $run_id,
                                          rel.relation_type = $rel_type, rel.is_observed = $is_observed
                            MERGE (r)-[:PRODUCED_ASSERTION]->(rel)
                            MERGE (rel)-[:SUPPORTED_BY_EVIDENCE]->(e)
                            WITH rel
                            MATCH (src:EntityCandidate {graph_entity_id: $src_gid})
                            MATCH (tgt:EntityCandidate {graph_entity_id: $tgt_gid})
                            MERGE (rel)-[:SOURCE_ENTITY]->(src)
                            MERGE (rel)-[:TARGET_ENTITY]->(tgt)
                            "#,
                            serde_json::json!({
                                "run_id": run_id,
                                "evidence_id": evidence_id,
                                "assertion_id": assertion_id,
                                "case_id": case_id,
                                "rel_type": rel_type,
                                "is_observed": is_observed,
                                "src_gid": src_gid,
                                "tgt_gid": tgt_gid,
                            }),
                        ));
                    }
                }
            }

            "RECONSTRUCTION_COMPLETED" => {
                let case_id = event.case_id.to_string();
                let recon_id = event.aggregate_id.to_string();
                let recon_type = p
                    .get("reconstruction_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("CONTRADICTION");
                let status = p
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("SUCCESS");

                statements.push(CypherStatement::new(
                    r#"
                    MERGE (c:Case {case_id: $case_id})
                    MERGE (rec:ReconstructionRun {reconstruction_run_id: $recon_id})
                    ON CREATE SET rec.case_id = $case_id, rec.reconstruction_type = $recon_type, rec.status = $status
                    MERGE (c)-[:PRODUCED_RECONSTRUCTION]->(rec)
                    "#,
                    serde_json::json!({
                        "case_id": case_id,
                        "recon_id": recon_id,
                        "recon_type": recon_type,
                        "status": status,
                    }),
                ));

                // Contradiction leads (support both "leads" and "contradictions")
                let leads = p
                    .get("leads")
                    .or_else(|| p.get("contradictions"))
                    .and_then(|v| v.as_array());
                if let Some(leads) = leads {
                    for lead in leads {
                        let contradiction_id = lead
                            .get("contradiction_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&recon_id);
                        let lead_type = lead
                            .get("lead_type")
                            .or_else(|| lead.get("conflict_type"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("SPATIAL_TRANSIT_IMPOSSIBLE");
                        let severity = lead
                            .get("severity")
                            .and_then(|v| v.as_str())
                            .unwrap_or("HIGH");
                        let explanation = lead
                            .get("neutral_explanation")
                            .or_else(|| lead.get("description"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let ev_a = lead
                            .get("event_a_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let ev_b = lead
                            .get("event_b_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        statements.push(CypherStatement::new(
                            r#"
                            MERGE (rec:ReconstructionRun {reconstruction_run_id: $recon_id})
                            MERGE (lead:ContradictionLead {contradiction_id: $contradiction_id})
                            ON CREATE SET lead.case_id = $case_id, lead.reconstruction_run_id = $recon_id,
                                          lead.contradiction_type = $lead_type, lead.severity = $severity,
                                          lead.neutral_explanation = $explanation
                            MERGE (rec)-[:IDENTIFIED_CONTRADICTION]->(lead)
                            WITH lead
                            MATCH (ev1:EventCandidate {event_id: $ev_a})
                            MATCH (ev2:EventCandidate {event_id: $ev_b})
                            MERGE (lead)-[:CONTRADICTS_EVENT]->(ev1)
                            MERGE (lead)-[:CONTRADICTS_EVENT]->(ev2)
                            "#,
                            serde_json::json!({
                                "recon_id": recon_id,
                                "contradiction_id": contradiction_id,
                                "case_id": case_id,
                                "lead_type": lead_type,
                                "severity": severity,
                                "explanation": explanation,
                                "ev_a": ev_a,
                                "ev_b": ev_b,
                            }),
                        ));
                    }
                }
            }

            _ => {
                debug!(event_type = %event.event_type, "Ignoring unknown or unprojectable outbox event type");
            }
        }

        Ok(statements)
    }
}
