//! Assurance routes: lead robustness and chain-of-custody audit.
//!
//! * `GET /api/v1/cases/:case_id/robustness` returns `CaseRobustnessReport`
//!   ([`crate::reconstruction::robustness`]).
//! * `GET /api/v1/cases/:case_id/custody/verify` returns `CaseCustodyAudit`
//!   ([`crate::custody_audit`]).
//!
//! Both read the latest stored reconstruction or evidence; neither changes anything.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use super::reconstruction::load_or_compute_baseline;
use crate::{app::AppState, error::AppError};
/// Breaking-point analysis of every contradiction lead (`GET /api/v1/cases/:case_id/robustness`).
pub async fn get_case_robustness(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;
    let reconstruction = load_or_compute_baseline(&state, case_id).await?;
    let report = crate::reconstruction::robustness::analyze_case_robustness(&reconstruction);
    Ok((StatusCode::OK, Json(report)))
}

/// Chain-of-custody audit and case seal (`GET /api/v1/cases/:case_id/custody/verify`).
pub async fn get_case_custody_audit(
    State(state): State<AppState>,
    Path(case_id_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    use crate::custody_audit::{
        audit_ledger, exhibit_verified, summarize, ExhibitCustodyAudit, LedgerAudit,
    };
    let case_id = Uuid::parse_str(&case_id_str).map_err(|_| {
        AppError::BadRequest(format!("Invalid case UUID format: '{}'", case_id_str))
    })?;
    let storage = state.ingestion_service.storage();

    // Exhibits of the case: relational store when connected, else the evidence storage.
    let mut evidence_ids: Vec<String> = Vec::new();
    if let Some(ref repo) = state.evidence_repo {
        if let Ok(entities) = repo.list_evidence_for_case(case_id).await {
            evidence_ids = entities
                .into_iter()
                .map(|e| e.evidence_id.to_string())
                .collect();
        }
    }
    if evidence_ids.is_empty() {
        evidence_ids = storage
            .list_manifests_for_case(&case_id_str)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.receipt.evidence_id)
            .collect();
    }
    evidence_ids.sort();
    evidence_ids.dedup();

    let mut exhibits = Vec::with_capacity(evidence_ids.len());
    for evidence_id in evidence_ids {
        let (filename, recorded, stored) =
            crate::reconstruction::traceability::resolve_evidence_integrity(
                &case_id_str,
                &evidence_id,
                storage,
                state.evidence_repo.as_deref(),
            )
            .await;
        let ledger = match (&state.custody_repo, Uuid::parse_str(&evidence_id)) {
            (Some(repo), Ok(uid)) => audit_ledger(&repo.list_custody_for_evidence(uid).await?),
            _ => LedgerAudit::unavailable(),
        };
        exhibits.push(ExhibitCustodyAudit {
            verified: exhibit_verified(stored, &ledger),
            evidence_id,
            original_filename: filename,
            recorded_sha256: recorded,
            stored_file: stored,
            ledger,
        });
    }
    let mode = if state.custody_repo.is_some() {
        "POSTGRES_LEDGER"
    } else {
        "FILESYSTEM_NO_LEDGER"
    };
    Ok((
        StatusCode::OK,
        Json(summarize(&case_id_str, mode, exhibits)),
    ))
}
