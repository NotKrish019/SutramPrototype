//! Axum Route Handlers for Forensic Reconstruction and Contradiction Analysis.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    app::AppState,
    error::AppError,
    extraction::model::ExtractedEventCandidate,
    reconstruction::{
        adapters::event_candidate_to_timeline_event_with_context, model::GeoPoint,
        service::ReconstructionOptions, temporal::RawTimelineEvent,
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
    let events: Vec<RawTimelineEvent> = if let Some(raw_list) = req.raw_events {
        if !raw_list.is_empty() {
            raw_list
        } else {
            load_case_events(&state, case_id, &options).await?
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
            load_case_events(&state, case_id, &options).await?
        }
    } else {
        load_case_events(&state, case_id, &options).await?
    };

    info!(
        case_id = %case_id,
        event_count = events.len(),
        "Executing full case reconstruction"
    );

    // 2. Execute reconstruction
    let reconstruction = state
        .reconstruction_service
        .reconstruct_case(&events, &options)?;

    // 3. Persist atomically to PostgreSQL and enqueue graph outbox projection if database is active
    let run_id = Uuid::new_v4();
    if let Some(ref repo) = state.reconstruction_repo {
        let params_json = serde_json::to_value(&options).unwrap_or_default();
        if let Err(e) = repo
            .save_full_reconstruction_atomic(
                case_id,
                run_id,
                None,
                "FULL_CASE_INTEGRATED",
                &params_json,
                &reconstruction,
            )
            .await
        {
            warn!(
                case_id = %case_id,
                error = %e,
                "Failed to persist reconstruction run to relational store (proceeding with result response)"
            );
        }
    }

    Ok((StatusCode::OK, Json(reconstruction)))
}

/// Retrieves the latest full reconstruction result for a case.
pub async fn get_case_reconstruction(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;

    // 1. Try relational store first
    if let Some(ref repo) = state.reconstruction_repo {
        if let Ok(Some(latest)) = repo.get_latest_reconstruction(case_id).await {
            return Ok((StatusCode::OK, Json(latest)));
        }
    }

    // 2. If not stored, compute dynamically from existing events
    let options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        ..Default::default()
    };
    let events = load_case_events(&state, case_id, &options).await?;
    let reconstruction = state
        .reconstruction_service
        .reconstruct_case(&events, &options)?;

    Ok((StatusCode::OK, Json(reconstruction)))
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

/// Helper to load case events from the database or fall back to filesystem derived storage.
async fn load_case_events(
    state: &AppState,
    case_id: Uuid,
    options: &ReconstructionOptions,
) -> Result<Vec<RawTimelineEvent>, AppError> {
    if let Some(ref repo) = state.reconstruction_repo {
        let entities = repo.get_events_for_case(case_id).await?;
        if !entities.is_empty() {
            let events: Vec<RawTimelineEvent> = entities
                .into_iter()
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

                    RawTimelineEvent {
                        event_id: e.event_id.to_string(),
                        source_evidence_id: e.evidence_id.to_string(),
                        timestamp: e.nominal_timestamp.to_rfc3339(),
                        timestamp_precision: e.timestamp_precision,
                        duration_seconds: e.duration_seconds,
                        uncertainty_window_start: e
                            .uncertainty_window_start
                            .map(|d| d.to_rfc3339()),
                        uncertainty_window_end: e.uncertainty_window_end.map(|d| d.to_rfc3339()),
                        coordinates: coords,
                        raw_description: raw_desc,
                        entity_ids,
                        metadata: e.payload,
                        source_timezone: Some(options.default_timezone.clone()),
                        clock_offset_seconds: Some(options.default_clock_offset_seconds),
                    }
                })
                .collect();
            return Ok(events);
        }
    }

    // Filesystem derived storage fallback for local/offline operations
    let storage = state.ingestion_service.storage();
    let case_id_str = case_id.to_string();
    let derived_case_dir = storage.derived_dir().join(&case_id_str);
    if derived_case_dir.exists() {
        if let Ok(mut read_dir) = tokio::fs::read_dir(&derived_case_dir).await {
            let mut all_events = Vec::new();
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    if file_type.is_dir() {
                        let ev_name = entry.file_name().to_string_lossy().to_string();
                        if let Ok(Some(pointer)) = storage
                            .load_active_run_pointer(&case_id_str, &ev_name)
                            .await
                        {
                            if let Ok(result) = storage
                                .load_run_result(&case_id_str, &ev_name, &pointer.active_run_id)
                                .await
                            {
                                for candidate in result.event_candidates {
                                    let timeline_event =
                                        event_candidate_to_timeline_event_with_context(
                                            &candidate,
                                            Some("EXACT_SECOND"),
                                            Some(&options.default_timezone),
                                            Some(options.default_clock_offset_seconds),
                                        );
                                    all_events.push(timeline_event);
                                }
                            }
                        }
                    }
                }
            }
            if !all_events.is_empty() {
                return Ok(all_events);
            }
        }
    }

    Ok(Vec::new())
}
