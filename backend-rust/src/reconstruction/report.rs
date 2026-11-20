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

    md.push_str("## 4. Forensic Disclaimer\n\n");
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
        forensic_disclaimer: FORENSIC_REPORT_DISCLAIMER.to_string(),
        markdown_dossier: md,
    }
}
