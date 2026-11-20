//! Forensic Case Report Generator (Member 4).
//!
//! Synthesizes multimodal forensic exhibits, unified extraction candidates,
//! chronological timeline reconstruction, spatial trajectories, and contradiction leads
//! into a comprehensive, court-admissible forensic dossier.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ingestion::EvidenceManifest;
use crate::persistence::model::CaseEntity;
use crate::reconstruction::model::ExportSafeguardStatus;
use crate::reconstruction::service::FullCaseReconstructionResult;

/// Judicial export clearance status.
pub const STATUS_SHIP_EXPORT_BLOCKED: &str = "SHIP_EXPORT_BLOCKED";
pub const STATUS_SHIP_EXPORT_CLEARED: &str = "SHIP_EXPORT_CLEARED";

/// Standard forensic disclaimer for judicial reports.
pub const FORENSIC_REPORT_DISCLAIMER: &str =
    "This analytical report compiles observed digital evidence exhibits, cryptographic hashes, \
     normalized timelines, and derived spatial contradictions. Analytical leads indicate spatio-temporal \
     discrepancies against investigative thresholds and do NOT constitute conclusive legal proof of human identity, \
     guilt, or factual perjury.";

/// Exhibit item in evidence inventory report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEvidenceItem {
    pub evidence_id: String,
    pub original_filename: String,
    pub source_type: String,
    pub mime_type: String,
    pub file_size_bytes: u64,
    pub sha256_hash: String,
    pub ingested_at: String,
}

/// Summary metrics for case report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseReportSummary {
    pub evidence_count: usize,
    pub total_timeline_events: usize,
    pub strictly_ordered_events: usize,
    pub concurrent_uncertain_events: usize,
    pub excluded_events_count: usize,
    pub spatial_trajectories_count: usize,
    pub total_movement_segments: usize,
    pub flagged_speed_segments: usize,
    pub open_contradictions_count: usize,
}

/// Item in contradiction section of report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportContradictionItem {
    pub lead_id: String,
    pub lead_type: String,
    pub entity_id: String,
    pub neutral_explanation: String,
    pub limitations: Vec<String>,
    pub generated_at: String,
}

/// One reconstructed timeline event with its provenance (Phase 3E.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportTimelineItem {
    pub event_id: String,
    pub source_evidence_id: String,
    /// The producer's timestamp, verbatim.
    pub raw_timestamp: String,
    pub nominal_utc: String,
    pub window_start_utc: String,
    pub window_end_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Number of values this event rests on that were not observed.
    pub assumptions_count: usize,
    /// Video provenance, present for frame-derived events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_pts_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_artifact_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_video_sha256: Option<String>,
}

/// Complete judicial forensic case report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseForensicReport {
    pub report_id: String,
    pub case_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fir_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub investigator_id: Option<String>,
    pub generated_at: String,
    pub judicial_export_status: String,
    pub judicial_export_reason: String,
    pub summary: CaseReportSummary,
    pub evidence_inventory: Vec<ReportEvidenceItem>,
    pub contradiction_leads: Vec<ReportContradictionItem>,
    /// Reconstructed events with provenance, in timeline order.
    #[serde(default)]
    pub timeline_events: Vec<ReportTimelineItem>,
    /// Candidates excluded from the timeline, with reasons.
    #[serde(default)]
    pub excluded_events: Vec<crate::reconstruction::model::ExcludedTimelineEvent>,
    /// Video frame handoff outcome per video exhibit.
    #[serde(default)]
    pub video_handoff: Vec<crate::reconstruction::video_handoff::VideoHandoffRecord>,
    /// Evidence gaps identifying missing corroboration, unlocated events, uncalibrated clocks, etc.
    #[serde(default)]
    pub evidence_gaps: Vec<crate::reconstruction::gaps::EvidenceGap>,
    pub forensic_disclaimer: String,
    pub markdown_dossier: String,
}

/// Generates a structured case forensic report and markdown dossier.
pub fn generate_case_report(
    case_id: &str,
    case_meta: Option<&CaseEntity>,
    exhibits: &[EvidenceManifest],
    reconstruction: &FullCaseReconstructionResult,
) -> CaseForensicReport {
    let title = case_meta
        .map(|c| c.title.clone())
        .unwrap_or_else(|| format!("Case {}", case_id));
    let fir_number = case_meta.and_then(|c| c.fir_number.clone());
    let investigator_id = case_meta.and_then(|c| c.investigator_id.clone());

    let report_id = Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("report:{}:{}", case_id, exhibits.len()).as_bytes(),
    )
    .to_string();

    let now_utc = Utc::now().to_rfc3339();

    // 1. Evidence Inventory
    let evidence_inventory: Vec<ReportEvidenceItem> = exhibits
        .iter()
        .map(|m| ReportEvidenceItem {
            evidence_id: m.receipt.evidence_id.clone(),
            original_filename: m.receipt.original_filename.clone(),
            source_type: m.receipt.source_type.clone(),
            mime_type: m.receipt.mime_type.clone(),
            file_size_bytes: m.receipt.file_size_bytes,
            sha256_hash: m.receipt.sha256_hash.clone(),
            ingested_at: m.receipt.created_at.clone(),
        })
        .collect();

    // 2. Contradictions
    let contradiction_leads: Vec<ReportContradictionItem> = reconstruction
        .contradictions
        .leads
        .iter()
        .map(|l| ReportContradictionItem {
            lead_id: l.lead_id.clone(),
            lead_type: format!("{:?}", l.lead_type),
            entity_id: l.entity_id.clone(),
            neutral_explanation: l.neutral_explanation.clone(),
            limitations: l.limitations.clone(),
            generated_at: l.generated_at.to_rfc3339(),
        })
        .collect();

    // 3. Judicial Export Safeguard Evaluation
    let has_contradictions = !reconstruction.contradictions.leads.is_empty();
    let has_excluded = !reconstruction.timeline.excluded_events.is_empty();

    let (judicial_export_status, judicial_export_reason) = if has_contradictions {
        (
            STATUS_SHIP_EXPORT_BLOCKED.to_string(),
            format!(
                "Judicial export is blocked pending human investigator resolution of {} unresolved contradiction lead(s).",
                reconstruction.contradictions.leads.len()
            ),
        )
    } else if has_excluded {
        (
            STATUS_SHIP_EXPORT_BLOCKED.to_string(),
            format!(
                "Judicial export is blocked because {} timeline event candidate(s) were excluded due to invalid timestamps or unknown timezones.",
                reconstruction.timeline.excluded_events.len()
            ),
        )
    } else if reconstruction.export_safeguard.status == ExportSafeguardStatus::Blocked {
        // Findings rest on values that were not observed (see timeline entry assumptions).
        (
            STATUS_SHIP_EXPORT_BLOCKED.to_string(),
            format!(
                "Judicial export is blocked: {}",
                reconstruction.export_safeguard.reasons.join(" ")
            ),
        )
    } else {
        (
            STATUS_SHIP_EXPORT_CLEARED.to_string(),
            "All evidence exhibits verified, timeline reconstructed with zero physical contradictions."
                .to_string(),
        )
    };

    let total_segments: usize = reconstruction
        .spatial
        .trajectories
        .iter()
        .map(|t| t.segments.len())
        .sum();

    let flagged_segments: usize = reconstruction
        .spatial
        .trajectories
        .iter()
        .map(|t| {
            t.segments
                .iter()
                .filter(|s| s.transit_metrics.exceeds_configured_speed_threshold)
                .count()
        })
        .sum();

    let summary = CaseReportSummary {
        evidence_count: exhibits.len(),
        total_timeline_events: reconstruction.timeline.total_events,
        strictly_ordered_events: reconstruction.timeline.strictly_ordered_count,
        concurrent_uncertain_events: reconstruction.timeline.concurrent_uncertain_count,
        excluded_events_count: reconstruction.timeline.excluded_events.len(),
        spatial_trajectories_count: reconstruction.spatial.trajectories.len(),
        total_movement_segments: total_segments,
        flagged_speed_segments: flagged_segments,
        open_contradictions_count: reconstruction.contradictions.leads.len(),
    };

    // 4. Build Markdown Dossier
    let mut md = String::new();
    md.push_str(&format!("# FORENSIC CASE DOSSIER: {}\n\n", title));
    md.push_str(&format!("**Case ID:** `{}`  \n", case_id));
    if let Some(ref fir) = fir_number {
        md.push_str(&format!("**FIR / Ref Number:** `{}`  \n", fir));
    }
    if let Some(ref inv) = investigator_id {
        md.push_str(&format!("**Lead Investigator:** `{}`  \n", inv));
    }
    md.push_str(&format!("**Report Generated:** `{}`  \n", now_utc));
    md.push_str(&format!(
        "**Judicial Export Status:** **`{}`**  \n\n",
        judicial_export_status
    ));

    md.push_str(&format!("> [!NOTE]\n> {}\n\n", judicial_export_reason));

    md.push_str("## 1. Executive Summary\n\n");
    md.push_str(&format!(
        "- **Evidence Exhibits:** {}\n\
         - **Reconstructed Timeline Events:** {} ({} strictly ordered, {} concurrent/uncertain)\n\
         - **Excluded Event Candidates:** {}\n\
         - **Spatial Trajectories:** {} entities tracked across {} movement segments\n\
         - **Speed Anomalies Flagged:** {}\n\
         - **Open Contradiction Leads:** {}\n\n",
        summary.evidence_count,
        summary.total_timeline_events,
        summary.strictly_ordered_events,
        summary.concurrent_uncertain_events,
        summary.excluded_events_count,
        summary.spatial_trajectories_count,
        summary.total_movement_segments,
        summary.flagged_speed_segments,
        summary.open_contradictions_count
    ));

    md.push_str("## 2. Chain of Custody & Evidence Inventory\n\n");
    if exhibits.is_empty() {
        md.push_str("_No exhibits registered in case dossier._\n\n");
    } else {
        md.push_str(
            "| Exhibit ID | Original Filename | Modality | Size (Bytes) | SHA-256 Digest |\n",
        );
        md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
        for e in &evidence_inventory {
            md.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | `{}` |\n",
                e.evidence_id, e.original_filename, e.source_type, e.file_size_bytes, e.sha256_hash
            ));
        }
        md.push('\n');
    }

    md.push_str("## 3. Contradiction Analysis & Investigative Leads\n\n");
    if contradiction_leads.is_empty() {
        md.push_str("✅ _No physical transit or temporal activity contradictions detected across reconstructed events._\n\n");
    } else {
        for (idx, lead) in contradiction_leads.iter().enumerate() {
            md.push_str(&format!(
                "### Lead {}: {} (Entity `{}`)\n\n",
                idx + 1,
                lead.lead_type,
                lead.entity_id
            ));
            md.push_str(&format!("- **Lead ID:** `{}`\n", lead.lead_id));
            md.push_str(&format!(
                "- **Explanation:** {}\n",
                lead.neutral_explanation
            ));
            md.push_str("- **Methodological Limitations & Alternative Explanations:**\n");
            for lim in &lead.limitations {
                md.push_str(&format!("  - {}\n", lim));
            }
            md.push('\n');
        }
    }

    // Timeline with provenance (including video frame provenance).
    let timeline_events: Vec<ReportTimelineItem> = reconstruction
        .timeline
        .entries
        .iter()
        .map(|e| {
            let meta = &e.metadata;
            let s = |k: &str| meta.get(k).and_then(|v| v.as_str()).map(str::to_string);
            ReportTimelineItem {
                event_id: e.event_id.clone(),
                source_evidence_id: e.source_evidence_id.clone(),
                raw_timestamp: e.raw_timestamp.clone(),
                nominal_utc: e.time_window.nominal_utc.to_rfc3339(),
                window_start_utc: e.time_window.window_start_utc.to_rfc3339(),
                window_end_utc: e.time_window.window_end_utc.to_rfc3339(),
                description: e.raw_description.clone(),
                assumptions_count: e.assumptions.len(),
                frame_sha256: s("sha256_hash"),
                frame_pts_seconds: meta.get("pts_seconds").and_then(|v| v.as_f64()),
                frame_artifact_reference: s("frame_artifact_reference"),
                source_video_sha256: s("source_video_sha256"),
            }
        })
        .collect();

    md.push_str("## 4. Reconstructed Timeline & Provenance\n\n");
    if timeline_events.is_empty() {
        md.push_str("_No events were reconstructed._\n\n");
    } else {
        md.push_str(
            "| Event | Exhibit | Raw timestamp | Nominal UTC | Assumptions | Frame artifact (SHA-256, PTS) |\n",
        );
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");
        for t in &timeline_events {
            let frame = match (
                &t.frame_artifact_reference,
                &t.frame_sha256,
                t.frame_pts_seconds,
            ) {
                (Some(r), Some(h), Some(p)) => format!("`{}` (`{}`, {:.3}s)", r, h, p),
                _ => "—".to_string(),
            };
            md.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
                t.event_id,
                t.source_evidence_id,
                t.raw_timestamp,
                t.nominal_utc,
                t.assumptions_count,
                frame
            ));
        }
        md.push('\n');
    }
    let excluded_events = reconstruction.timeline.excluded_events.clone();
    if !excluded_events.is_empty() {
        md.push_str("### Excluded event candidates\n\n");
        for x in &excluded_events {
            md.push_str(&format!(
                "- `{}` (exhibit `{}`): {}\n",
                x.event_id, x.source_evidence_id, x.reason
            ));
        }
        md.push('\n');
    }
    let video_handoff = reconstruction.video_handoff.clone();
    if !video_handoff.is_empty() {
        md.push_str("### Video frame handoff\n\n");
        for v in &video_handoff {
            md.push_str(&format!(
                "- Exhibit `{}` (source SHA-256 `{}`): `{:?}`, {} frame(s) extracted, {} handed off{}{}\n",
                v.evidence_id,
                v.source_video_sha256,
                v.status,
                v.frames_extracted,
                v.candidates_handed_off,
                v.frames_reference
                    .as_ref()
                    .map(|r| format!(", frames `{}`", r))
                    .unwrap_or_default(),
                v.anchor_basis
                    .as_ref()
                    .map(|b| format!("; anchor: {}", b))
                    .unwrap_or_default(),
            ));
            for d in &v.diagnostics {
                md.push_str(&format!("  - {}\n", d));
            }
        }
        md.push('\n');
    }

    let evidence_gaps = if !reconstruction.evidence_gaps.is_empty() {
        reconstruction.evidence_gaps.clone()
    } else {
        crate::reconstruction::gaps::analyze_evidence_gaps(case_id, reconstruction, exhibits, None).gaps
    };

    if !evidence_gaps.is_empty() {
        md.push_str("## 5. Evidence Gap Intelligence\n\n");
        md.push_str("Deterministic audit of missing corroboration, unlocated events, uncalibrated device clocks, and provenance gaps:\n\n");
        for gap in &evidence_gaps {
            md.push_str(&format!(
                "- **[{}]** `{}` on `{}:{}`: {}\n  - _Impact:_ {}\n  - _Suggested Corroboration:_ {}\n",
                gap.severity.as_str(),
                gap.gap_type.as_str(),
                gap.subject_type,
                gap.subject_id,
                gap.explanation,
                gap.impact_on_reconstruction,
                gap.suggested_corroborating_evidence
            ));
        }
        md.push('\n');
    }

    md.push_str("## 6. Forensic Disclaimer\n\n");
    md.push_str(&format!("_{}_\n", FORENSIC_REPORT_DISCLAIMER));

    CaseForensicReport {
        report_id,
        case_id: case_id.to_string(),
        title,
        fir_number,
        investigator_id,
        generated_at: now_utc,
        judicial_export_status,
        judicial_export_reason,
        summary,
        evidence_inventory,
        contradiction_leads,
        timeline_events,
        excluded_events,
        video_handoff,
        evidence_gaps,
        forensic_disclaimer: FORENSIC_REPORT_DISCLAIMER.to_string(),
        markdown_dossier: md,
    }
}
