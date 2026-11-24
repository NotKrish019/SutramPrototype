//! Axum Route Handlers for Forensic Reconstruction and Contradiction Analysis.

use std::collections::BTreeSet;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::{
    app::AppState,
    error::AppError,
    extraction::model::ExtractedEventCandidate,
    ingestion::model::EvidenceManifest,
    reconstruction::{
        adapters::{
            event_candidate_to_timeline_event_with_context, unified_extraction_to_timeline_events,
        },
        model::GeoPoint,
        model::{AssumptionKind, ReconstructionAssumption},
        scenario::{simulate_scenario, validate_scenario_assumptions, ScenarioSimulationRequest},
        service::ReconstructionOptions,
        temporal::RawTimelineEvent,
        traceability::{resolve_case_reconstruction_provenance, resolve_finding_provenance},
        video_handoff::{
            handoff_case_video_frames, VideoHandoffRecord, DEFAULT_FRAME_TIMEOUT,
            DEFAULT_MAX_FRAMES,
        },
        FullCaseReconstructionResult, TimelineEntry,
    },
};

/// Request payload to trigger case reconstruction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerReconstructionRequest {
    #[serde(default)]
    pub options: Option<ReconstructionOptions>,
    #[serde(default)]
    pub event_candidates: Option<Vec<ExtractedEventCandidate>>,
    #[serde(default)]
    pub raw_events: Option<Vec<RawTimelineEvent>>,
}

/// Triggers full timeline, spatial trajectory, and contradiction reconstruction for a case.
///
/// If input events are provided in the payload, they are used directly.
/// Otherwise, events are loaded from the PostgreSQL database or filesystem derived storage.
pub async fn trigger_case_reconstruction(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
    Json(payload): Json<Option<TriggerReconstructionRequest>>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    let req = payload.unwrap_or_default();
    let mut options = req.options.unwrap_or_default();
    options.case_id = Some(case_id.to_string());

    // 1. Gather input events
    let mut video_records: Vec<VideoHandoffRecord> = Vec::new();
    let mut contributing_run_ids: Vec<Uuid> = Vec::new();
    let events: Vec<RawTimelineEvent> = if let Some(raw_list) = req.raw_events {
        if !raw_list.is_empty() {
            raw_list
        } else {
            let (events, records, run_ids) =
                load_case_events_with_video(&state, case_id, &options).await?;
            video_records = records;
            contributing_run_ids = run_ids;
            events
        }
    } else if let Some(candidates) = req.event_candidates {
        if !candidates.is_empty() {
            candidates
                .iter()
                .map(|c| {
                    event_candidate_to_timeline_event_with_context(
                        c,
                        Some("EXACT_SECOND"),
                        Some(&options.default_timezone),
                        Some(options.default_clock_offset_seconds),
                    )
                })
                .collect()
        } else {
            let (events, records, run_ids) =
                load_case_events_with_video(&state, case_id, &options).await?;
            video_records = records;
            contributing_run_ids = run_ids;
            events
        }
    } else {
        let (events, records, run_ids) =
            load_case_events_with_video(&state, case_id, &options).await?;
        video_records = records;
        contributing_run_ids = run_ids;
        events
    };

    info!(
        case_id = %case_id,
        event_count = events.len(),
        "Executing full case reconstruction"
    );

    // 2. Execute reconstruction
    let mut reconstruction = state
        .reconstruction_service
        .reconstruct_case(&events, &options)?;
    reconstruction.attach_video_handoff(video_records);

    let run_id = Uuid::new_v4();
    let run_id_str = run_id.to_string();
    reconstruction.reconstruction_run_id = Some(run_id_str.clone());
    reconstruction.run_id = Some(run_id_str);

    // 3. Persist to filesystem derived store (ensuring offline and restart survival)
    save_reconstruction_to_fs(&state, case_id, &reconstruction).await?;

    // 4. Persist atomically to PostgreSQL and enqueue graph outbox projection if database is active
    if let Some(ref repo) = state.reconstruction_repo {
        let params_json = serde_json::to_value(&options).unwrap_or_default();
        repo.save_full_reconstruction_atomic(
            case_id,
            run_id,
            &contributing_run_ids,
            "FULL_CASE_INTEGRATED",
            &params_json,
            &reconstruction,
        )
        .await?;
    }

    Ok((StatusCode::OK, Json(reconstruction)))
}

/// Helper to load the baseline reconstruction from relational store, filesystem, or dynamic compute.
pub async fn load_or_compute_baseline(
    state: &AppState,
    case_id: Uuid,
) -> Result<FullCaseReconstructionResult, AppError> {
    // 1. Try relational store first
    if let Some(ref repo) = state.reconstruction_repo {
        if let Some(latest) = repo.get_latest_reconstruction(case_id).await? {
            return Ok(latest);
        }
    }

    // 2. Try filesystem derived store
    if let Some(latest) = load_latest_reconstruction_from_fs(state, case_id).await {
        return Ok(latest);
    }

    // 3. If not stored, compute dynamically from existing events
    let options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        ..Default::default()
    };
    let (events, video_records, _) = load_case_events_with_video(state, case_id, &options).await?;
    let mut reconstruction = state
        .reconstruction_service
        .reconstruct_case(&events, &options)?;
    reconstruction.attach_video_handoff(video_records);
    let r_id = format!("base-{}", Uuid::new_v4());
    reconstruction.reconstruction_run_id = Some(r_id.clone());
    reconstruction.run_id = Some(r_id);

    Ok(reconstruction)
}

/// Retrieves the latest full reconstruction result for a case.
pub async fn get_case_reconstruction(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    let reconstruction = load_or_compute_baseline(&state, case_id).await?;
    Ok((StatusCode::OK, Json(reconstruction)))
}

/// Retrieves the complete evidence-to-claim provenance trace for a specific reconstruction finding.
pub async fn get_finding_provenance(
    State(state): State<AppState>,
    Path((case_id_str, finding_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    let reconstruction = load_or_compute_baseline(&state, case_id).await?;
    let storage = state.ingestion_service.storage();

    let trace = resolve_finding_provenance(
        &case_id_str,
        &finding_id,
        &reconstruction,
        storage,
        state.evidence_repo.as_deref(),
    )
    .await?;

    Ok((StatusCode::OK, Json(trace)))
}

/// Retrieves the comprehensive evidence-to-claim provenance report for all findings in a case.
pub async fn get_case_reconstruction_provenance(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    let reconstruction = load_or_compute_baseline(&state, case_id).await?;
    let storage = state.ingestion_service.storage();

    let report = resolve_case_reconstruction_provenance(
        &case_id_str,
        &reconstruction,
        storage,
        state.evidence_repo.as_deref(),
    )
    .await?;

    Ok((StatusCode::OK, Json(report)))
}

/// Retrieves the comprehensive evidence gap analysis report for a case.
pub async fn get_case_evidence_gaps(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    // 1. Load or compute baseline reconstruction
    let reconstruction = load_or_compute_baseline(&state, case_id).await?;

    // 2. Load manifests from DB or storage
    let exhibits = if let Some(ref repo) = state.evidence_repo {
        if let Ok(entities) = repo.list_evidence_for_case(case_id).await {
            entities
                .into_iter()
                .map(|e| EvidenceManifest {
                    receipt: crate::ingestion::EvidenceUploadReceipt {
                        evidence_id: e.evidence_id.to_string(),
                        case_id: e.case_id.to_string(),
                        original_filename: e.original_filename,
                        source_type: e.source_type,
                        mime_type: e.mime_type,
                        file_size_bytes: e.file_size_bytes as u64,
                        sha256_hash: e.sha256_hash,
                        storage_uri: e.storage_uri,
                        status: crate::ingestion::IngestionStatus::Ingested,
                        metadata: e.metadata,
                        created_at: e
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    },
                    relative_raw_path: format!(
                        "{}/{}/{}.raw",
                        case_id, e.evidence_id, e.evidence_id
                    ),
                    stored_at: e
                        .created_at
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                })
                .collect()
        } else {
            state
                .ingestion_service
                .storage()
                .list_manifests_for_case(&case_id_str)
                .await
                .unwrap_or_default()
        }
    } else {
        state
            .ingestion_service
            .storage()
            .list_manifests_for_case(&case_id_str)
            .await
            .unwrap_or_default()
    };

    let report = crate::reconstruction::gaps::analyze_evidence_gaps(
        &case_id_str,
        &reconstruction,
        &exhibits,
        None,
    );

    Ok((StatusCode::OK, Json(report)))
}

/// Retrieves timeline reconstruction results for a case.
pub async fn get_case_timeline(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    if let Some(ref repo) = state.reconstruction_repo {
        if let Ok(Some(latest)) = repo.get_latest_reconstruction(case_id).await {
            return Ok((StatusCode::OK, Json(latest.timeline)));
        }
    }

    if let Some(latest) = load_latest_reconstruction_from_fs(&state, case_id).await {
        return Ok((StatusCode::OK, Json(latest.timeline)));
    }

    let options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        ..Default::default()
    };
    let events = load_case_events(&state, case_id, &options).await?;
    let reconstruction = state
        .reconstruction_service
        .reconstruct_case(&events, &options)?;

    Ok((StatusCode::OK, Json(reconstruction.timeline)))
}

/// Retrieves spatial reconstruction results (trajectories and overlaps) for a case.
pub async fn get_case_spatial(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    if let Some(ref repo) = state.reconstruction_repo {
        if let Ok(Some(latest)) = repo.get_latest_reconstruction(case_id).await {
            return Ok((StatusCode::OK, Json(latest.spatial)));
        }
    }

    if let Some(latest) = load_latest_reconstruction_from_fs(&state, case_id).await {
        return Ok((StatusCode::OK, Json(latest.spatial)));
    }

    let options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        ..Default::default()
    };
    let events = load_case_events(&state, case_id, &options).await?;
    let reconstruction = state
        .reconstruction_service
        .reconstruct_case(&events, &options)?;

    Ok((StatusCode::OK, Json(reconstruction.spatial)))
}

/// Retrieves contradiction analysis leads for a case.
pub async fn get_case_contradictions(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    if let Some(ref repo) = state.reconstruction_repo {
        if let Ok(Some(latest)) = repo.get_latest_reconstruction(case_id).await {
            return Ok((StatusCode::OK, Json(latest.contradictions)));
        }
    }

    if let Some(latest) = load_latest_reconstruction_from_fs(&state, case_id).await {
        return Ok((StatusCode::OK, Json(latest.contradictions)));
    }

    let options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        ..Default::default()
    };
    let events = load_case_events(&state, case_id, &options).await?;
    let reconstruction = state
        .reconstruction_service
        .reconstruct_case(&events, &options)?;

    Ok((StatusCode::OK, Json(reconstruction.contradictions)))
}

/// Converts a reconstructed timeline entry back to a RawTimelineEvent for scenario reprocessing.
fn timeline_entry_to_raw_event(entry: &TimelineEntry) -> RawTimelineEvent {
    RawTimelineEvent {
        event_id: entry.event_id.clone(),
        source_evidence_id: entry.source_evidence_id.clone(),
        timestamp: if !entry.raw_timestamp.is_empty() {
            entry.raw_timestamp.clone()
        } else {
            entry.time_window.nominal_utc.to_rfc3339()
        },
        timestamp_precision: entry.time_window.precision.as_str().to_string(),
        duration_seconds: entry.time_window.duration_seconds,
        uncertainty_window_start: Some(entry.time_window.window_start_utc.to_rfc3339()),
        uncertainty_window_end: Some(entry.time_window.window_end_utc.to_rfc3339()),
        coordinates: entry.coordinates,
        raw_description: entry.raw_description.clone(),
        entity_ids: entry.entity_ids.clone(),
        metadata: entry.metadata.clone(),
        source_timezone: Some("UTC".to_string()),
        clock_offset_seconds: Some(0.0),
        assumptions: entry.assumptions.clone(),
    }
}

/// Executes a stateless what-if reconstruction simulation under alternative assumptions.
///
/// Baseline records, raw evidence, and official reconstructions are strictly immutable
/// and never modified or overwritten by this endpoint.
pub async fn simulate_case_scenario(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
    Json(payload): Json<ScenarioSimulationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    validate_scenario_assumptions(&payload.assumptions)?;

    let baseline_options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        ..Default::default()
    };

    // 1. Gather input events
    let mut video_records: Vec<VideoHandoffRecord> = Vec::new();
    let events: Vec<RawTimelineEvent> = if let Some(ref raw_list) = payload.raw_events {
        if !raw_list.is_empty() {
            raw_list.clone()
        } else {
            let (loaded_events, v_recs, _) =
                load_case_events_with_video(&state, case_id, &baseline_options).await?;
            video_records = v_recs;
            loaded_events
        }
    } else {
        let (loaded_events, v_recs, _) =
            load_case_events_with_video(&state, case_id, &baseline_options).await?;
        video_records = v_recs;
        loaded_events
    };

    // 2. Load baseline reconstruction (relational store -> filesystem -> dynamic fallback)
    let baseline = if let Some(ref repo) = state.reconstruction_repo {
        if let Some(latest) = repo.get_latest_reconstruction(case_id).await? {
            latest
        } else if let Some(latest) = load_latest_reconstruction_from_fs(&state, case_id).await {
            latest
        } else {
            if events.is_empty() {
                return Err(AppError::NotFound(format!(
                    "No evidence events found for case '{}'",
                    case_id
                )));
            }
            let mut b = state
                .reconstruction_service
                .reconstruct_case(&events, &baseline_options)?;
            b.attach_video_handoff(video_records.clone());
            let r_id = format!("base-{}", Uuid::new_v4());
            b.reconstruction_run_id = Some(r_id.clone());
            b.run_id = Some(r_id);
            b
        }
    } else if let Some(latest) = load_latest_reconstruction_from_fs(&state, case_id).await {
        latest
    } else {
        if events.is_empty() {
            return Err(AppError::NotFound(format!(
                "No evidence events found for case '{}'",
                case_id
            )));
        }
        let mut b = state
            .reconstruction_service
            .reconstruct_case(&events, &baseline_options)?;
        b.attach_video_handoff(video_records.clone());
        let r_id = format!("base-{}", Uuid::new_v4());
        b.reconstruction_run_id = Some(r_id.clone());
        b.run_id = Some(r_id);
        b
    };

    let final_events = if events.is_empty() {
        if baseline.timeline.entries.is_empty() {
            return Err(AppError::NotFound(format!(
                "No evidence events found for case '{}'",
                case_id
            )));
        }
        baseline
            .timeline
            .entries
            .iter()
            .map(timeline_entry_to_raw_event)
            .collect()
    } else {
        events
    };

    let response = simulate_scenario(
        &case_id_str,
        &baseline,
        &final_events,
        &video_records,
        &baseline_options,
        &payload,
        &state.reconstruction_service,
    )?;

    // NOTE: Neither save_reconstruction_to_fs nor repo.save_full_reconstruction_atomic
    // is called here. The scenario execution is strictly stateless and in-memory.

    Ok((StatusCode::OK, Json(response)))
}

/// Helper to load case events from the database or fall back to filesystem derived storage.
async fn load_case_events(
    state: &AppState,
    case_id: Uuid,
    options: &ReconstructionOptions,
) -> Result<Vec<RawTimelineEvent>, AppError> {
    load_case_events_with_video(state, case_id, options)
        .await
        .map(|(events, _, _)| events)
}

/// Loads case events, including frame events handed off from video exhibits (Phase 3E.3), and
/// returns one handoff record per video exhibit.
async fn load_case_events_with_video(
    state: &AppState,
    case_id: Uuid,
    _options: &ReconstructionOptions,
) -> Result<(Vec<RawTimelineEvent>, Vec<VideoHandoffRecord>, Vec<Uuid>), AppError> {
    if let Some(ref repo) = state.reconstruction_repo {
        let entities = repo.get_events_for_case(case_id).await?;
        if !entities.is_empty() {
            let events: Vec<RawTimelineEvent> = entities
                .iter()
                .map(|e| {
                    let coords = if let (Some(lat), Some(lon)) = (e.latitude, e.longitude) {
                        GeoPoint::new(lat, lon).ok()
                    } else {
                        None
                    };

                    let entity_ids = e
                        .involved_entity_ids
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();

                    let raw_desc = e
                        .payload
                        .get("location_name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let mut meta = e.payload.clone();
                    if let serde_json::Value::Object(ref mut map) = meta {
                        if !e.source_record_id.trim().is_empty() {
                            map.entry("source_record_id".to_string())
                                .or_insert_with(|| serde_json::Value::String(e.source_record_id.clone()));
                        }
                        map.entry("extraction_run_id".to_string())
                            .or_insert_with(|| serde_json::Value::String(e.extraction_run_id.to_string()));
                        map.entry("is_observed".to_string())
                            .or_insert_with(|| serde_json::Value::Bool(e.is_observed));
                    }

                    RawTimelineEvent {
                        event_id: e.event_id.to_string(),
                        source_evidence_id: e.evidence_id.to_string(),
                        timestamp: e.nominal_timestamp.to_rfc3339(),
                        timestamp_precision: e.timestamp_precision.clone(),
                        duration_seconds: e.duration_seconds,
                        uncertainty_window_start: e
                            .uncertainty_window_start
                            .map(|d| d.to_rfc3339()),
                        uncertainty_window_end: e.uncertainty_window_end.map(|d| d.to_rfc3339()),
                        coordinates: coords,
                        raw_description: raw_desc,
                        entity_ids,
                        metadata: meta,
                        source_timezone: Some("UTC".to_string()),
                        clock_offset_seconds: Some(0.0),
                        // Stored events are already in UTC; do not apply default_clock_offset_seconds again.
                        assumptions: vec![
                            ReconstructionAssumption {
                                kind: AssumptionKind::ClockOffsetNotDeclaredOnEvent,
                                field: "clock_offset_seconds".to_string(),
                                assumed_value: "0.0".to_string(),
                                basis: "Event nominal timestamp is already stored as UTC in relational store; offset not double-applied."
                                    .to_string(),
                            },
                        ],
                    }
                })
                .collect();

            // Check for video exhibits and hand off frames in database mode
            let (frame_candidates, video_records) = {
                let storage = state.ingestion_service.storage();
                let case_id_str = case_id.to_string();
                let manifests = if let Some(ref ev_repo) = state.evidence_repo {
                    let db_evidence = ev_repo
                        .list_evidence_for_case(case_id)
                        .await
                        .unwrap_or_default();
                    if !db_evidence.is_empty() {
                        db_evidence
                            .into_iter()
                            .map(|e| EvidenceManifest {
                                receipt: crate::ingestion::EvidenceUploadReceipt {
                                    evidence_id: e.evidence_id.to_string(),
                                    case_id: e.case_id.to_string(),
                                    original_filename: e.original_filename,
                                    source_type: e.source_type,
                                    mime_type: e.mime_type,
                                    file_size_bytes: e.file_size_bytes as u64,
                                    sha256_hash: e.sha256_hash,
                                    storage_uri: e.storage_uri,
                                    status: match e.ingestion_status.as_str() {
                                        "INGESTED" => crate::ingestion::IngestionStatus::Ingested,
                                        "EXTRACTION_PENDING" => {
                                            crate::ingestion::IngestionStatus::ExtractionPending
                                        }
                                        "EXTRACTED" => crate::ingestion::IngestionStatus::Extracted,
                                        "EXTRACTION_FAILED" => {
                                            crate::ingestion::IngestionStatus::ExtractionFailed
                                        }
                                        "CORRUPTED" => crate::ingestion::IngestionStatus::Corrupted,
                                        _ => crate::ingestion::IngestionStatus::Ingested,
                                    },
                                    metadata: e.metadata,
                                    created_at: e
                                        .created_at
                                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                                },
                                relative_raw_path: format!(
                                    "{}/{}/{}.raw",
                                    case_id_str, e.evidence_id, e.evidence_id
                                ),
                                stored_at: e
                                    .created_at
                                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                            })
                            .collect()
                    } else {
                        storage
                            .list_manifests_for_case(&case_id_str)
                            .await
                            .unwrap_or_default()
                    }
                } else {
                    storage
                        .list_manifests_for_case(&case_id_str)
                        .await
                        .unwrap_or_default()
                };
                handoff_case_video_frames(
                    storage,
                    &case_id_str,
                    &manifests,
                    DEFAULT_MAX_FRAMES,
                    DEFAULT_FRAME_TIMEOUT,
                )
                .await
            };

            let mut all_events = events;
            all_events.extend(frame_candidates.iter().map(|c| {
                event_candidate_to_timeline_event_with_context(c, Some("EXACT_SECOND"), None, None)
            }));

            let contributing_run_ids: Vec<Uuid> = entities
                .iter()
                .map(|e| e.extraction_run_id)
                .collect::<BTreeSet<Uuid>>()
                .into_iter()
                .collect();

            return Ok((all_events, video_records, contributing_run_ids));
        }
    }

    // Filesystem derived storage fallback for local/offline operations
    let storage = state.ingestion_service.storage();
    let case_id_str = case_id.to_string();
    let derived_case_dir = storage.derived_dir().join(&case_id_str);
    let mut all_events = Vec::new();
    let mut contributing_run_ids = BTreeSet::new();

    if derived_case_dir.exists() {
        if let Ok(mut read_dir) = tokio::fs::read_dir(&derived_case_dir).await {
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    if file_type.is_dir() {
                        let ev_name = entry.file_name().to_string_lossy().to_string();
                        if let Ok(Some(pointer)) = storage
                            .load_active_run_pointer(&case_id_str, &ev_name)
                            .await
                        {
                            if let Ok(uid) = Uuid::parse_str(&pointer.active_run_id) {
                                contributing_run_ids.insert(uid);
                            }
                            if let Ok(result) = storage
                                .load_run_result(&case_id_str, &ev_name, &pointer.active_run_id)
                                .await
                            {
                                // Use the exhibit's declared timezone / clock offset recorded on
                                // the extraction run. Request defaults would silently replace a
                                // declared calibration (Phase 3E.1 fix).
                                let mut events_from_run = unified_extraction_to_timeline_events(
                                    &result,
                                    Some("EXACT_SECOND"),
                                );
                                for ev in &mut events_from_run {
                                    if let serde_json::Value::Object(ref mut map) = ev.metadata {
                                        map.entry("extraction_run_id".to_string()).or_insert_with(
                                            || {
                                                serde_json::Value::String(
                                                    pointer.active_run_id.clone(),
                                                )
                                            },
                                        );
                                    }
                                }
                                all_events.extend(events_from_run);
                            }
                        }
                    }
                }
            }
        }
    }

    // Video exhibits: extract (or reuse) frames, anchor from declared calibration, and hand
    // them to Member 1's consumer. Frame events carry their own declared timezone/offset.
    let manifests = storage
        .list_manifests_for_case(&case_id_str)
        .await
        .unwrap_or_default();
    let (frame_candidates, video_records) = handoff_case_video_frames(
        storage,
        &case_id_str,
        &manifests,
        DEFAULT_MAX_FRAMES,
        DEFAULT_FRAME_TIMEOUT,
    )
    .await;
    all_events.extend(frame_candidates.iter().map(|c| {
        event_candidate_to_timeline_event_with_context(c, Some("EXACT_SECOND"), None, None)
    }));
    all_events.sort_by(|a, b| a.event_id.cmp(&b.event_id));

    Ok((
        all_events,
        video_records,
        contributing_run_ids.into_iter().collect(),
    ))
}

/// Helper to persist the latest reconstruction run to filesystem derived storage for offline/restart continuity.
async fn save_reconstruction_to_fs(
    state: &AppState,
    case_id: Uuid,
    reconstruction: &FullCaseReconstructionResult,
) -> Result<(), AppError> {
    let recon_dir = state
        .ingestion_service
        .storage()
        .derived_dir()
        .join(case_id.to_string())
        .join("reconstruction");
    tokio::fs::create_dir_all(&recon_dir).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to create reconstruction directory: {}",
            e
        ))
    })?;
    let file_path = recon_dir.join("latest.json");
    let json_bytes = serde_json::to_vec_pretty(reconstruction).map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "Failed to serialize reconstruction result: {}",
            e
        ))
    })?;
    tokio::fs::write(&file_path, json_bytes)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to write latest.json: {}", e)))?;
    Ok(())
}

/// Helper to load the latest reconstruction run from filesystem derived storage if available.
async fn load_latest_reconstruction_from_fs(
    state: &AppState,
    case_id: Uuid,
) -> Option<FullCaseReconstructionResult> {
    let file_path = state
        .ingestion_service
        .storage()
        .derived_dir()
        .join(case_id.to_string())
        .join("reconstruction")
        .join("latest.json");
    if file_path.exists() {
        if let Ok(bytes) = tokio::fs::read(&file_path).await {
            return serde_json::from_slice(&bytes).ok();
        }
    }
    None
}

/// Generates and returns a judicial forensic report for a case (Member 4).
pub async fn get_case_report(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    // 1. Load case metadata if available
    let case_entity = if let Some(ref repo) = state.case_repo {
        repo.get_case(case_id).await.unwrap_or(None)
    } else {
        None
    };

    // 2. Load evidence manifests (checking relational evidence repository first if available)
    let exhibits = if let Some(ref repo) = state.evidence_repo {
        let db_evidence = repo
            .list_evidence_for_case(case_id)
            .await
            .unwrap_or_default();
        if !db_evidence.is_empty() {
            db_evidence
                .into_iter()
                .map(|e| EvidenceManifest {
                    receipt: crate::ingestion::EvidenceUploadReceipt {
                        evidence_id: e.evidence_id.to_string(),
                        case_id: e.case_id.to_string(),
                        original_filename: e.original_filename,
                        source_type: e.source_type,
                        mime_type: e.mime_type,
                        file_size_bytes: e.file_size_bytes as u64,
                        sha256_hash: e.sha256_hash,
                        storage_uri: e.storage_uri,
                        status: match e.ingestion_status.as_str() {
                            "INGESTED" => crate::ingestion::IngestionStatus::Ingested,
                            "EXTRACTION_PENDING" => {
                                crate::ingestion::IngestionStatus::ExtractionPending
                            }
                            "EXTRACTED" => crate::ingestion::IngestionStatus::Extracted,
                            "EXTRACTION_FAILED" => {
                                crate::ingestion::IngestionStatus::ExtractionFailed
                            }
                            "CORRUPTED" => crate::ingestion::IngestionStatus::Corrupted,
                            _ => crate::ingestion::IngestionStatus::Ingested,
                        },
                        metadata: e.metadata,
                        created_at: e
                            .created_at
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    },
                    relative_raw_path: format!(
                        "{}/{}/{}.raw",
                        case_id, e.evidence_id, e.evidence_id
                    ),
                    stored_at: e
                        .created_at
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                })
                .collect()
        } else {
            state
                .ingestion_service
                .storage()
                .list_manifests_for_case(&case_id.to_string())
                .await
                .unwrap_or_default()
        }
    } else {
        state
            .ingestion_service
            .storage()
            .list_manifests_for_case(&case_id.to_string())
            .await
            .unwrap_or_default()
    };

    // 3. Load or compute reconstruction
    let options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        ..Default::default()
    };
    let reconstruction = if let Some(ref repo) = state.reconstruction_repo {
        if let Ok(Some(latest)) = repo.get_latest_reconstruction(case_id).await {
            latest
        } else if let Some(latest) = load_latest_reconstruction_from_fs(&state, case_id).await {
            latest
        } else {
            let (events, video_records, _) =
                load_case_events_with_video(&state, case_id, &options).await?;
            let mut computed = state
                .reconstruction_service
                .reconstruct_case(&events, &options)?;
            computed.attach_video_handoff(video_records);
            computed
        }
    } else if let Some(latest) = load_latest_reconstruction_from_fs(&state, case_id).await {
        latest
    } else {
        let (events, video_records, _) =
            load_case_events_with_video(&state, case_id, &options).await?;
        let mut computed = state
            .reconstruction_service
            .reconstruct_case(&events, &options)?;
        computed.attach_video_handoff(video_records);
        computed
    };

    let report = crate::reconstruction::report::generate_case_report(
        &case_id.to_string(),
        case_entity.as_ref(),
        &exhibits,
        &reconstruction,
    );

    Ok((StatusCode::OK, Json(report)))
}
