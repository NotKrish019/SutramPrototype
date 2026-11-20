//! Extraction Run Repository: Database access and transactional persistence for analytical runs.

use sqlx::PgPool;
use uuid::Uuid;

use super::model::ExtractionRunEntity;
use crate::error::AppError;
use crate::extraction::{ExtractionRunManifest, ExtractionRunStatus, UnifiedExtractionResult};

#[derive(Debug, Clone)]
pub struct ExtractionRunRepository {
    pool: PgPool,
}

impl ExtractionRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Records an initial extraction run in PENDING or RUNNING status.
    pub async fn create_run(
        &self,
        run: &ExtractionRunEntity,
    ) -> Result<ExtractionRunEntity, AppError> {
        let row = sqlx::query_as::<_, ExtractionRunEntity>(
            r#"
            INSERT INTO extraction_runs (
                extraction_run_id, evidence_id, case_id, source_sha256,
                extractor_name, extractor_version, configuration_hash, configuration,
                status, is_active, started_at, completed_at, duration_ms,
                records_processed, valid_records_count, rejected_records_count,
                validation_issue_count, result_sha256, result_snapshot, error_message, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
            RETURNING *
            "#,
        )
        .bind(run.extraction_run_id)
        .bind(run.evidence_id)
        .bind(run.case_id)
        .bind(&run.source_sha256)
        .bind(&run.extractor_name)
        .bind(&run.extractor_version)
        .bind(&run.configuration_hash)
        .bind(&run.configuration)
        .bind(&run.status)
        .bind(run.is_active)
        .bind(run.started_at)
        .bind(run.completed_at)
        .bind(run.duration_ms)
        .bind(run.records_processed)
        .bind(run.valid_records_count)
        .bind(run.rejected_records_count)
        .bind(run.validation_issue_count)
        .bind(&run.result_sha256)
        .bind(&run.result_snapshot)
        .bind(&run.error_message)
        .bind(run.created_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to record extraction run: {}", e)))?;

        Ok(row)
    }

    /// Retrieves an extraction run by its UUID.
    pub async fn get_run(&self, run_id: Uuid) -> Result<Option<ExtractionRunEntity>, AppError> {
        let row = sqlx::query_as::<_, ExtractionRunEntity>(
            r#"
            SELECT * FROM extraction_runs WHERE extraction_run_id = $1
            "#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to fetch extraction run: {}", e))
        })?;

        Ok(row)
    }

    /// Lists all historical extraction runs for an evidence exhibit ordered chronologically.
    pub async fn list_runs_for_evidence(
        &self,
        evidence_id: Uuid,
    ) -> Result<Vec<ExtractionRunEntity>, AppError> {
        let rows = sqlx::query_as::<_, ExtractionRunEntity>(
            r#"
            SELECT * FROM extraction_runs WHERE evidence_id = $1 ORDER BY started_at ASC
            "#,
        )
        .bind(evidence_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to list extraction runs: {}", e))
        })?;

        Ok(rows)
    }

    /// Retrieves the currently active extraction run for an evidence exhibit.
    pub async fn get_active_run_for_evidence(
        &self,
        evidence_id: Uuid,
    ) -> Result<Option<ExtractionRunEntity>, AppError> {
        let row = sqlx::query_as::<_, ExtractionRunEntity>(
            r#"
            SELECT * FROM extraction_runs WHERE evidence_id = $1 AND is_active = TRUE
            "#,
        )
        .bind(evidence_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to fetch active extraction run: {}",
                e
            ))
        })?;

        Ok(row)
    }

    /// Atomically persists complete extraction results, snapshots, and normalized children.
    ///
    /// Executes within a single transaction:
    /// 1. Updates extraction_runs record with finalized telemetry and JSONB snapshot.
    /// 2. Inserts normalized identifiers, entities, events, relationships, and validation issues.
    /// 3. If promote_active is true, acquires row-level lock on evidence and activates the run.
    pub async fn complete_run_transactional(
        &self,
        run_manifest: &ExtractionRunManifest,
        result: &UnifiedExtractionResult,
        promote_active: bool,
    ) -> Result<(), AppError> {
        let run_id = Uuid::parse_str(&run_manifest.extraction_run_id)
            .map_err(|e| AppError::BadRequest(format!("Invalid extraction_run_id UUID: {}", e)))?;
        let evidence_id = Uuid::parse_str(&run_manifest.evidence_id)
            .map_err(|e| AppError::BadRequest(format!("Invalid evidence_id UUID: {}", e)))?;
        let case_id = Uuid::parse_str(&run_manifest.case_id)
            .map_err(|e| AppError::BadRequest(format!("Invalid case_id UUID: {}", e)))?;

        let completed_at = run_manifest
            .completed_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        let mut tx = self.pool.begin().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to begin transaction: {}", e))
        })?;

        let snapshot_json = serde_json::to_value(result).map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to serialize result snapshot: {}",
                e
            ))
        })?;

        let records_processed = i32::try_from(run_manifest.records_processed).map_err(|_| {
            AppError::BadRequest(format!(
                "records_processed ({}) exceeds maximum 32-bit integer capacity",
                run_manifest.records_processed
            ))
        })?;
        let valid_records_count =
            i32::try_from(run_manifest.valid_records_count).map_err(|_| {
                AppError::BadRequest(format!(
                    "valid_records_count ({}) exceeds maximum 32-bit integer capacity",
                    run_manifest.valid_records_count
                ))
            })?;
        let rejected_records_count =
            i32::try_from(run_manifest.rejected_records_count).map_err(|_| {
                AppError::BadRequest(format!(
                    "rejected_records_count ({}) exceeds maximum 32-bit integer capacity",
                    run_manifest.rejected_records_count
                ))
            })?;
        let validation_issue_count =
            i32::try_from(run_manifest.validation_issue_count).map_err(|_| {
                AppError::BadRequest(format!(
                    "validation_issue_count ({}) exceeds maximum 32-bit integer capacity",
                    run_manifest.validation_issue_count
                ))
            })?;

        // 1. Update extraction run row
        sqlx::query(
            r#"
            UPDATE extraction_runs
            SET status = $2,
                completed_at = $3,
                duration_ms = $4,
                records_processed = $5,
                valid_records_count = $6,
                rejected_records_count = $7,
                validation_issue_count = $8,
                result_sha256 = $9,
                result_snapshot = $10,
                error_message = $11
            WHERE extraction_run_id = $1
            "#,
        )
        .bind(run_id)
        .bind(run_manifest.status.to_string())
        .bind(completed_at)
        .bind(run_manifest.duration_ms)
        .bind(records_processed)
        .bind(valid_records_count)
        .bind(rejected_records_count)
        .bind(validation_issue_count)
        .bind(&run_manifest.result_sha256)
        .bind(&snapshot_json)
        .bind(&run_manifest.error_message)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to update extraction run: {}", e))
        })?;

        // 2. Insert normalized identifiers
        for ident in &result.identifiers {
            sqlx::query(
                r#"
                INSERT INTO extracted_identifiers (
                    extraction_run_id, evidence_id, case_id, source_record_id,
                    identifier_type, raw_value, normalized_value, is_ambiguous, location_reference
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(run_id)
            .bind(evidence_id)
            .bind(case_id)
            .bind(&ident.source_record_id)
            .bind(&ident.identifier_type)
            .bind(&ident.raw_value)
            .bind(&ident.normalized_value)
            .bind(ident.is_ambiguous)
            .bind(&ident.location_reference)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to insert identifier: {}", e))
            })?;
        }

        // 3. Insert resolved entities
        for ent in &result.resolved_entities {
            let ent_id = Uuid::new_v4();
            let obs_json = serde_json::to_value(&ent.raw_mentions).unwrap_or_default();
            let refs_json = serde_json::to_value(&ent.source_evidence_ids).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO resolved_entities (
                    entity_id, extraction_run_id, case_id, entity_type,
                    canonical_value, observed_identifiers, confidence, evidence_references
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(ent_id)
            .bind(run_id)
            .bind(case_id)
            .bind(&ent.identifier_type)
            .bind(&ent.canonical_identifier)
            .bind(&obs_json)
            .bind(1.0f64)
            .bind(&refs_json)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to insert resolved entity: {}", e))
            })?;
        }

        // 4. Insert events
        for ev in &result.event_candidates {
            let ev_id = Uuid::parse_str(&ev.candidate_id).unwrap_or_else(|_| {
                super::model::deterministic_uuid(&case_id, ev.candidate_id.as_bytes())
            });
            let nominal_dt = chrono::DateTime::parse_from_rfc3339(&ev.raw_timestamp)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or(completed_at);
            let entities_json =
                serde_json::to_value(&ev.involved_entity_raw_ids).unwrap_or_default();

            let mut coords_inferred = false;
            let (lat, lon) = if let Some(coords) = ev.metadata.get("coordinates") {
                (
                    coords.get("lat").and_then(|v| v.as_f64()),
                    coords.get("lon").and_then(|v| v.as_f64()),
                )
            } else if let (Some(la), Some(lo)) = (
                ev.metadata.get("latitude").and_then(|v| v.as_f64()),
                ev.metadata.get("longitude").and_then(|v| v.as_f64()),
            ) {
                (Some(la), Some(lo))
            } else if let Some(tower) = ev
                .metadata
                .get("cell_tower_id")
                .and_then(|v| v.as_str())
                .or(ev.location_name.as_deref())
            {
                if let Some(geo) = crate::reconstruction::resolve_cell_tower_centroid(tower) {
                    coords_inferred = true;
                    (Some(geo.lat), Some(geo.lon))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let mut payload_map = match serde_json::to_value(&ev.metadata).unwrap_or_default() {
                serde_json::Value::Object(m) => m,
                _ => serde_json::Map::new(),
            };
            payload_map.insert(
                "candidate_id".to_string(),
                serde_json::json!(ev.candidate_id),
            );
            if let Some(ref loc) = ev.location_name {
                payload_map.insert("location_name".to_string(), serde_json::json!(loc));
            }
            if coords_inferred {
                payload_map.insert("coordinates_inferred".to_string(), serde_json::json!(true));
                payload_map.insert(
                    "coordinates_inference_basis".to_string(),
                    serde_json::json!("Representative centroid looked up from cell tower identifier"),
                );
            }
            let is_observed = !coords_inferred
                && ev
                    .metadata
                    .get("is_observed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
            let payload_json = serde_json::Value::Object(payload_map);

            sqlx::query(
                r#"
                INSERT INTO events (
                    event_id, extraction_run_id, evidence_id, case_id, source_record_id,
                    event_type, nominal_timestamp, timestamp_precision, duration_seconds,
                    uncertainty_window_start, uncertainty_window_end, latitude, longitude,
                    involved_entity_ids, payload, is_observed
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                "#,
            )
            .bind(ev_id)
            .bind(run_id)
            .bind(evidence_id)
            .bind(case_id)
            .bind(&ev.source_record_id)
            .bind(&ev.event_type)
            .bind(nominal_dt)
            .bind("EXACT_SECOND")
            .bind(ev.duration_seconds)
            .bind(None::<chrono::DateTime<chrono::Utc>>)
            .bind(None::<chrono::DateTime<chrono::Utc>>)
            .bind(lat)
            .bind(lon)
            .bind(&entities_json)
            .bind(&payload_json)
            .bind(is_observed)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to insert event: {}", e)))?;
        }

        // 5. Insert relationships
        for rel in &result.relationship_candidates {
            let rel_id = Uuid::new_v4();
            let refs_json = serde_json::to_value(&rel.evidence_basis).unwrap_or_default();
            let meta_json = serde_json::to_value(&rel.metadata).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO relationships (
                    relationship_id, extraction_run_id, case_id, source_id, source_kind,
                    target_id, target_kind, relation_type, weight, is_observed,
                    evidence_references, metadata
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                "#,
            )
            .bind(rel_id)
            .bind(run_id)
            .bind(case_id)
            .bind(&rel.source_raw_id)
            .bind("ENTITY")
            .bind(&rel.target_raw_id)
            .bind("ENTITY")
            .bind(&rel.relation_type)
            .bind(None::<f64>)
            .bind(rel.is_observed)
            .bind(&refs_json)
            .bind(&meta_json)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to insert relationship: {}", e))
            })?;
        }

        // 6. Insert validation issues
        for issue in &result.validation_issues {
            let iss_id = Uuid::new_v4();
            let row_idx = match issue.row_index {
                Some(r) => Some(i64::try_from(r).map_err(|_| {
                    AppError::BadRequest(format!(
                        "Validation issue row_index ({}) exceeds 64-bit integer capacity",
                        r
                    ))
                })?),
                None => None,
            };

            sqlx::query(
                r#"
                INSERT INTO validation_issues (
                    issue_id, extraction_run_id, evidence_id, case_id,
                    record_id, field_name, issue_type, raw_value, message, row_index
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(iss_id)
            .bind(run_id)
            .bind(evidence_id)
            .bind(case_id)
            .bind(&issue.record_id)
            .bind(&issue.field_name)
            .bind(&issue.issue_type)
            .bind(&issue.raw_value)
            .bind(&issue.message)
            .bind(row_idx)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to insert validation issue: {}", e))
            })?;
        }

        // 7. Active Run Management (only when promote_active is true and status is accepted)
        if promote_active
            && (run_manifest.status == ExtractionRunStatus::Success
                || run_manifest.status == ExtractionRunStatus::PartialSuccess)
        {
            // Lock evidence row
            sqlx::query("SELECT evidence_id FROM evidence WHERE evidence_id = $1 FOR UPDATE")
                .bind(evidence_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to acquire evidence lock: {}", e))
                })?;

            // Deactivate previous active run
            sqlx::query("UPDATE extraction_runs SET is_active = FALSE WHERE evidence_id = $1 AND is_active = TRUE")
                .bind(evidence_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to deactivate previous active run: {}", e)))?;

            // Activate new run
            sqlx::query("UPDATE extraction_runs SET is_active = TRUE WHERE extraction_run_id = $1")
                .bind(run_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to activate new run: {}", e))
                })?;
        }

        // 8. Enqueue graph projection outbox event within the same transaction
        if run_manifest.status == ExtractionRunStatus::Success
            || run_manifest.status == ExtractionRunStatus::PartialSuccess
        {
            let payload = serde_json::json!({
                "evidence_id": evidence_id.to_string(),
                "extractor_version": run_manifest.extractor_version,
                "configuration_hash": run_manifest.configuration_hash,
                "result_sha256": run_manifest.result_sha256,
                "status": run_manifest.status.to_string(),
                "duration_ms": run_manifest.duration_ms,
                "entities": result.resolved_entities,
                "events": result.event_candidates,
                "relationships": result.relationship_candidates,
            });

            let _ = crate::graph::GraphOutboxRepository::enqueue_transactional(
                &mut tx,
                case_id,
                "EXTRACTION_RUN",
                run_id,
                1,
                "EXTRACTION_COMPLETED",
                &payload,
            )
            .await;
        }

        tx.commit().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to commit extraction transaction: {}",
                e
            ))
        })?;

        Ok(())
    }
}
