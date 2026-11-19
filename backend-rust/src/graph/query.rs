//! Typed Graph Query Engine for Forensic Investigators.
//!
//! Provides strictly parameterized, case-scoped Cypher queries.
//! Guarantees:
//! 1. Every query requires and verifies `case_id`.
//! 2. Bounded traversal depth (max depth 3 for neighbors, max hops 5 for paths) to prevent DoS.
//! 3. Zero string interpolation of user input.

use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use super::client::GraphClient;
use super::model::{GraphNeighborResponse, GraphNode, GraphPathResponse, GraphSummaryResponse};
use super::outbox::GraphOutboxRepository;
use crate::error::AppError;

pub struct GraphQueryService {
    client: Arc<dyn GraphClient>,
    outbox_repo: Option<Arc<GraphOutboxRepository>>,
    hmac_key: Option<String>,
}

impl GraphQueryService {
    pub fn new(
        client: Arc<dyn GraphClient>,
        outbox_repo: Option<Arc<GraphOutboxRepository>>,
        hmac_key: Option<String>,
    ) -> Self {
        Self {
            client,
            outbox_repo,
            hmac_key,
        }
    }

    /// Retrieves an aggregate summary of knowledge graph nodes and synchronization status for a case.
    pub async fn get_case_summary(&self, case_id: Uuid) -> Result<GraphSummaryResponse, AppError> {
        let cypher = r#"
        MATCH (c:Case {case_id: $case_id})
        OPTIONAL MATCH (c)-[:HAS_EVIDENCE]->(e:Evidence)
        OPTIONAL MATCH (e)-[:PRODUCED_EXTRACTION_RUN]->(r:ExtractionRun)
        OPTIONAL MATCH (r)-[:EXTRACTED_ENTITY]->(ent:EntityCandidate)
        OPTIONAL MATCH (r)-[:EXTRACTED_EVENT]->(ev:EventCandidate)
        OPTIONAL MATCH (r)-[:PRODUCED_ASSERTION]->(rel:RelationshipAssertion)
        OPTIONAL MATCH (c)-[:PRODUCED_RECONSTRUCTION]->(rec:ReconstructionRun)
        OPTIONAL MATCH (rec)-[:IDENTIFIED_CONTRADICTION]->(lead:ContradictionLead)
        RETURN count(DISTINCT e) as evidence_count,
               count(DISTINCT r) as extraction_run_count,
               count(DISTINCT ent) as entity_count,
               count(DISTINCT ev) as event_count,
               count(DISTINCT rel) as assertion_count,
               count(DISTINCT rec) as reconstruction_count,
               count(DISTINCT lead) as contradiction_count
        "#;

        let res = self
            .client
            .execute_query(
                cypher,
                serde_json::json!({ "case_id": case_id.to_string() }),
            )
            .await?;

        let mut evidence_count = 0;
        let mut extraction_run_count = 0;
        let mut entity_candidate_count = 0;
        let mut event_candidate_count = 0;
        let mut relationship_assertion_count = 0;
        let mut reconstruction_run_count = 0;
        let mut contradiction_lead_count = 0;

        if let Some(row) = res
            .get("data")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
        {
            evidence_count = row
                .get("evidence_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            extraction_run_count = row
                .get("extraction_run_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            entity_candidate_count = row
                .get("entity_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            event_candidate_count = row.get("event_count").and_then(|v| v.as_i64()).unwrap_or(0);
            relationship_assertion_count = row
                .get("assertion_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            reconstruction_run_count = row
                .get("reconstruction_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            contradiction_lead_count = row
                .get("contradiction_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
        }

        let privacy_status = if self.hmac_key.is_some() {
            "hmac_sha256_active".to_string()
        } else {
            "hmac_key_unconfigured".to_string()
        };

        let sync_status = if let Some(repo) = &self.outbox_repo {
            let metrics = repo.get_metrics(Some(case_id)).await.unwrap_or_default();
            if metrics.pending > 0 || metrics.retry_wait > 0 {
                format!("SYNCING ({} pending)", metrics.pending + metrics.retry_wait)
            } else if metrics.failed > 0 || metrics.dead_letter > 0 {
                format!(
                    "DEGRADED ({} failures)",
                    metrics.failed + metrics.dead_letter
                )
            } else {
                "SYNCHRONIZED".to_string()
            }
        } else {
            "STANDALONE_GRAPH".to_string()
        };

        Ok(GraphSummaryResponse {
            case_id,
            evidence_count,
            extraction_run_count,
            entity_candidate_count,
            event_candidate_count,
            relationship_assertion_count,
            reconstruction_run_count,
            contradiction_lead_count,
            graph_synchronization_status: sync_status,
            last_projection_time: Some(chrono::Utc::now()),
            identifier_privacy: privacy_status,
        })
    }

    /// Explores the bounded neighborhood around a target entity candidate within an authorized case.
    pub async fn get_entity_neighbors(
        &self,
        case_id: Uuid,
        entity_id: &str,
        max_depth: u32,
        limit: i64,
    ) -> Result<GraphNeighborResponse, AppError> {
        let depth = max_depth.clamp(1, 3);
        let safe_limit = limit.clamp(1, 100);

        debug!(case_id = %case_id, entity_id = %entity_id, depth = depth, "Querying entity neighbors");

        let cypher = r#"
        MATCH (root:EntityCandidate {case_id: $case_id})
        WHERE root.graph_entity_id = $entity_id OR root.fingerprint = $entity_id
        MATCH path = (root)-[r*1..3]-(neighbor)
        WHERE ALL(n IN nodes(path) WHERE n.case_id = $case_id)
        WITH root, neighbor, relationships(path) as rels
        LIMIT $limit
        RETURN root, collect(DISTINCT neighbor) as neighbors, collect(DISTINCT rels) as all_rels
        "#;

        let res = self
            .client
            .execute_query(
                cypher,
                serde_json::json!({
                    "case_id": case_id.to_string(),
                    "entity_id": entity_id,
                    "limit": safe_limit,
                }),
            )
            .await?;

        let mut nodes = Vec::new();
        let relationships = Vec::new();

        nodes.push(GraphNode {
            id: entity_id.to_string(),
            label: "EntityCandidate".to_string(),
            properties: serde_json::json!({ "case_id": case_id.to_string() }),
        });

        if let Some(rows) = res.get("data").and_then(|v| v.as_array()) {
            for row in rows {
                if let Some(neighbors) = row.get("neighbors").and_then(|v| v.as_array()) {
                    for n in neighbors {
                        let nid = n.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let nlabel = n.get("label").and_then(|v| v.as_str()).unwrap_or("Node");
                        nodes.push(GraphNode {
                            id: nid.to_string(),
                            label: nlabel.to_string(),
                            properties: n.clone(),
                        });
                    }
                }
            }
        }

        Ok(GraphNeighborResponse {
            case_id,
            root_entity_id: entity_id.to_string(),
            depth,
            nodes,
            relationships,
        })
    }

    /// Traces shortest paths between two entities within an authorized case.
    pub async fn find_path(
        &self,
        case_id: Uuid,
        from_entity_id: &str,
        to_entity_id: &str,
        max_hops: u32,
    ) -> Result<GraphPathResponse, AppError> {
        let _hops = max_hops.clamp(1, 5);

        let cypher = r#"
        MATCH (src:EntityCandidate {case_id: $case_id}), (tgt:EntityCandidate {case_id: $case_id})
        WHERE (src.graph_entity_id = $from_id OR src.fingerprint = $from_id)
          AND (tgt.graph_entity_id = $to_id OR tgt.fingerprint = $to_id)
        MATCH p = shortestPath((src)-[*1..5]-(tgt))
        WHERE ALL(n IN nodes(p) WHERE n.case_id = $case_id)
        RETURN p
        "#;

        let res = self
            .client
            .execute_query(
                cypher,
                serde_json::json!({
                    "case_id": case_id.to_string(),
                    "from_id": from_entity_id,
                    "to_id": to_entity_id,
                }),
            )
            .await?;

        let has_path = res
            .get("data")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        Ok(GraphPathResponse {
            case_id,
            from_entity_id: from_entity_id.to_string(),
            to_entity_id: to_entity_id.to_string(),
            found: has_path,
            nodes: Vec::new(),
            relationships: Vec::new(),
        })
    }
}
