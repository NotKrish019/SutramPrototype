//! Stateless What-If Reconstruction Simulation Engine (Member 4).
//!
//! Evaluates alternative investigative assumptions (clock drift offsets,
//! spatial/temporal bounds, exhibit inclusion/exclusion) independently of
//! the official baseline reconstruction, without persisting or mutating
//! evidence or baseline records.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

use super::model::{
    AssumptionKind, ContradictionLead, ContradictionLeadType, ExportSafeguardStatus, GeoPoint,
    ReconstructionAssumption,
};
use super::service::{
    ForensicReconstructionService, FullCaseReconstructionResult, ReconstructionOptions,
};
use super::temporal::{validate_timezone, RawTimelineEvent};
use super::video_handoff::VideoHandoffRecord;
use crate::error::AppError;

/// Individual finding comparison item (unchanged, newly appearing, or disappearing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingComparisonItem {
    pub finding_id: String,
    pub finding_type: String,
    pub entity_id: String,
    pub event_ids: Vec<String>,
    pub summary: String,
    pub details: serde_json::Value,
}

/// Finding whose robustness or quantitative metrics shifted under alternative assumptions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingRobustnessDelta {
    pub finding_id: String,
    pub finding_type: String,
    pub entity_id: String,
    pub event_ids: Vec<String>,
    pub metric_name: String,
    pub baseline_value: f64,
    pub scenario_value: f64,
    pub baseline_metrics: serde_json::Value,
    pub scenario_metrics: serde_json::Value,
    pub explanation: String,
}

/// Structured comparison result between baseline and scenario findings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioComparisonResult {
    /// Findings persisting identically in both baseline and scenario.
    pub findings_unchanged: Vec<FindingComparisonItem>,
    /// Findings that were absent in baseline, but emerged under scenario assumptions.
    pub findings_newly_appearing: Vec<FindingComparisonItem>,
    /// Findings that existed in baseline, but resolved or disappeared under scenario assumptions.
    pub findings_disappearing: Vec<FindingComparisonItem>,
    /// Findings present in both, but whose metrics (speed, overlap) or robustness changed.
    pub findings_robustness_changed: Vec<FindingRobustnessDelta>,
    /// Human-readable list of alternative assumptions applied in this scenario.
    pub applied_assumptions_summary: Vec<String>,
    /// Plain-language forensic explanations for all observed differences.
    pub explanations: Vec<String>,
}

/// Request payload to execute a stateless what-if reconstruction simulation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioSimulationRequest {
    /// Optional investigative hypothesis or scenario title.
    #[serde(default)]
    pub scenario_name: Option<String>,
    /// Alternative assumptions and overrides to test.
    #[serde(default)]
    pub assumptions: ScenarioAssumptions,
    /// Optional raw events supplied directly in the request payload.
    #[serde(default)]
    pub raw_events: Option<Vec<RawTimelineEvent>>,
}

/// Explicit uncertainty window override for an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplicitWindowOverride {
    pub start_utc: String,
    pub end_utc: String,
}

/// Alternative investigative assumptions for what-if scenario testing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScenarioAssumptions {
    /// Alternative default clock offset in seconds (DEC-001).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_clock_offset_seconds: Option<f64>,

    /// Exhibit-specific clock offset overrides (evidence_id -> offset_seconds).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub exhibit_clock_offsets: HashMap<String, f64>,

    /// Alternative IANA timezone name (e.g. "Asia/Kolkata", "UTC").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_timezone: Option<String>,

    /// Alternative speed threshold in km/h for transit impossibility detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_threshold_kmh: Option<f64>,

    /// Alternative estimated timestamp uncertainty half-width in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_half_width_seconds: Option<f64>,

    /// Evidence exhibits to exclude from this scenario (evidence_id).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_evidence_ids: Vec<String>,

    /// Event candidates to exclude from this scenario (event_id).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_event_ids: Vec<String>,

    /// Explicit temporal uncertainty window overrides (event_id -> window).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub event_uncertainty_overrides: HashMap<String, ExplicitWindowOverride>,

    /// Spatial coordinate overrides (event_id -> GeoPoint).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub event_coordinate_overrides: HashMap<String, GeoPoint>,
}

/// Complete response for a stateless scenario simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSimulationResponse {
    /// Canonical identifier of the baseline reconstruction run.
    pub baseline_run_id: String,
    /// Ephemeral unique identifier for this simulation run.
    pub scenario_run_id: String,
    /// Friendly scenario name.
    pub scenario_name: String,
    /// Prominent flag: always true, confirming results are hypothetical exploratory findings.
    pub is_hypothetical: bool,
    /// UTC ISO 8601 timestamp when simulation was computed.
    pub simulated_at: String,
    /// The exact alternative assumptions applied.
    pub assumptions_applied: ScenarioAssumptions,
    /// Forensic comparison and delta analysis against baseline.
    pub comparison: ScenarioComparisonResult,
    /// Full simulated reconstruction results under applied assumptions.
    pub simulated_reconstruction: FullCaseReconstructionResult,
}

/// Validates that all supplied scenario assumptions are mathematically sound and finite.
pub fn validate_scenario_assumptions(assumptions: &ScenarioAssumptions) -> Result<(), AppError> {
    if let Some(offset) = assumptions.default_clock_offset_seconds {
        if !offset.is_finite() {
            return Err(AppError::BadRequest(
                "default_clock_offset_seconds must be a finite number".to_string(),
            ));
        }
    }

    for (evidence_id, offset) in &assumptions.exhibit_clock_offsets {
        if !offset.is_finite() {
            return Err(AppError::BadRequest(format!(
                "Clock offset for exhibit '{}' must be a finite number",
                evidence_id
            )));
        }
    }

    if let Some(ref tz) = assumptions.default_timezone {
        validate_timezone(tz)?;
    }

    if let Some(speed) = assumptions.speed_threshold_kmh {
        if !speed.is_finite() || speed <= 0.0 || speed > 10000.0 {
            return Err(AppError::BadRequest(
                "speed_threshold_kmh must be a positive finite number (<= 10,000 km/h)".to_string(),
            ));
        }
    }

    if let Some(hw) = assumptions.estimated_half_width_seconds {
        if !hw.is_finite() || hw <= 0.0 {
            return Err(AppError::BadRequest(
                "estimated_half_width_seconds must be a positive finite number".to_string(),
            ));
        }
    }

    for (ev_id, win) in &assumptions.event_uncertainty_overrides {
        let s = chrono::DateTime::parse_from_rfc3339(win.start_utc.trim()).map_err(|e| {
            AppError::BadRequest(format!(
                "Invalid start_utc '{}' for event '{}': {}",
                win.start_utc, ev_id, e
            ))
        })?;
        let e = chrono::DateTime::parse_from_rfc3339(win.end_utc.trim()).map_err(|err| {
            AppError::BadRequest(format!(
                "Invalid end_utc '{}' for event '{}': {}",
                win.end_utc, ev_id, err
            ))
        })?;
        if s >= e {
            return Err(AppError::BadRequest(format!(
                "Invalid uncertainty window for event '{}': start ({}) must be strictly earlier than end ({})",
                ev_id, win.start_utc, win.end_utc
            )));
        }
    }

    Ok(())
}

/// Executes a stateless what-if scenario simulation without mutating or persisting baseline evidence.
pub fn simulate_scenario(
    case_id: &str,
    baseline: &FullCaseReconstructionResult,
    events: &[RawTimelineEvent],
    video_records: &[VideoHandoffRecord],
    baseline_options: &ReconstructionOptions,
    req: &ScenarioSimulationRequest,
    service: &ForensicReconstructionService,
) -> Result<ScenarioSimulationResponse, AppError> {
    validate_scenario_assumptions(&req.assumptions)?;

    let scenario_run_id = format!("sim-{}", Uuid::new_v4());
    let scenario_name = req
        .scenario_name
        .clone()
        .unwrap_or_else(|| "What-If Hypothesis Simulation".to_string());

    let excluded_exhibits: HashSet<&str> = req
        .assumptions
        .excluded_evidence_ids
        .iter()
        .map(|s| s.as_str())
        .collect();
    let excluded_events: HashSet<&str> = req
        .assumptions
        .excluded_event_ids
        .iter()
        .map(|s| s.as_str())
        .collect();

    // 1. Prepare simulated events
    let mut sim_events = Vec::new();
    for ev in events {
        if excluded_exhibits.contains(ev.source_evidence_id.as_str()) {
            continue;
        }
        if excluded_events.contains(ev.event_id.as_str()) {
            continue;
        }

        let mut cloned_ev = ev.clone();

        // Apply coordinate override
        if let Some(coord) = req.assumptions.event_coordinate_overrides.get(&ev.event_id) {
            cloned_ev.coordinates = Some(*coord);
            cloned_ev.assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::CoordinatesInferredFromTowerId,
                field: "coordinates".to_string(),
                assumed_value: format!("{},{}", coord.lat, coord.lon),
                basis: "Investigator what-if scenario coordinate override (hypothetical position).".to_string(),
            });
        }

        // Apply uncertainty window override
        if let Some(win) = req.assumptions.event_uncertainty_overrides.get(&ev.event_id) {
            cloned_ev.uncertainty_window_start = Some(win.start_utc.clone());
            cloned_ev.uncertainty_window_end = Some(win.end_utc.clone());
            // Anchor nominal timestamp inside the explicit window if it fell outside
            if let (Ok(nom_dt), Ok(start_dt), Ok(end_dt)) = (
                chrono::DateTime::parse_from_rfc3339(cloned_ev.timestamp.trim()),
                chrono::DateTime::parse_from_rfc3339(win.start_utc.trim()),
                chrono::DateTime::parse_from_rfc3339(win.end_utc.trim()),
            ) {
                if nom_dt < start_dt || nom_dt >= end_dt {
                    cloned_ev.timestamp = win.start_utc.clone();
                }
            }
            cloned_ev.assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::PrecisionDefaulted,
                field: "uncertainty_window".to_string(),
                assumed_value: format!("[{}, {})", win.start_utc, win.end_utc),
                basis: "Investigator what-if scenario uncertainty window override.".to_string(),
            });
        }

        // Apply clock offset overrides
        if let Some(offset) = req.assumptions.exhibit_clock_offsets.get(&ev.source_evidence_id) {
            cloned_ev.clock_offset_seconds = Some(*offset);
            cloned_ev.assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::ClockOffsetNotDeclaredOnEvent,
                field: "clock_offset_seconds".to_string(),
                assumed_value: offset.to_string(),
                basis: format!(
                    "Investigator what-if scenario exhibit clock offset override applied to exhibit '{}'.",
                    ev.source_evidence_id
                ),
            });
        } else if let Some(def_offset) = req.assumptions.default_clock_offset_seconds {
            cloned_ev.clock_offset_seconds = Some(def_offset);
            cloned_ev.assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::ClockOffsetNotDeclaredOnEvent,
                field: "clock_offset_seconds".to_string(),
                assumed_value: def_offset.to_string(),
                basis: "Investigator what-if scenario default clock offset override.".to_string(),
            });
        }

        // Apply timezone override
        if let Some(ref tz) = req.assumptions.default_timezone {
            cloned_ev.source_timezone = Some(tz.clone());
            cloned_ev.assumptions.push(ReconstructionAssumption {
                kind: AssumptionKind::TimezoneNotDeclaredOnEvent,
                field: "source_timezone".to_string(),
                assumed_value: tz.clone(),
                basis: "Investigator what-if scenario default timezone override.".to_string(),
            });
        }

        sim_events.push(cloned_ev);
    }

    // 2. Prepare scenario reconstruction options
    let scenario_options = ReconstructionOptions {
        case_id: Some(case_id.to_string()),
        default_timezone: req
            .assumptions
            .default_timezone
            .clone()
            .unwrap_or_else(|| baseline_options.default_timezone.clone()),
        default_clock_offset_seconds: req
            .assumptions
            .default_clock_offset_seconds
            .unwrap_or(baseline_options.default_clock_offset_seconds),
        speed_threshold_kmh: req
            .assumptions
            .speed_threshold_kmh
            .unwrap_or(baseline_options.speed_threshold_kmh),
        estimated_half_width_seconds: req
            .assumptions
            .estimated_half_width_seconds
            .unwrap_or(baseline_options.estimated_half_width_seconds),
    };

    // 3. Execute scenario reconstruction
    let mut sim_reconstruction = service.reconstruct_case(&sim_events, &scenario_options)?;

    // Filter video handoff records
    let sim_video_records: Vec<VideoHandoffRecord> = video_records
        .iter()
        .filter(|r| !excluded_exhibits.contains(r.evidence_id.as_str()))
        .cloned()
        .collect();
    sim_reconstruction.attach_video_handoff(sim_video_records);

    // Enforce safety: Mark as hypothetical simulation
    sim_reconstruction.export_safeguard.status = ExportSafeguardStatus::Blocked;
    sim_reconstruction.export_safeguard.reasons.push(
        "Hypothetical scenario simulation: results are exploratory what-if projections and cannot be exported as judicial evidence.".to_string(),
    );
    sim_reconstruction.reconstruction_run_id = Some(scenario_run_id.clone());
    sim_reconstruction.run_id = Some(scenario_run_id.clone());

    // 4. Compare baseline and scenario
    let comparison = compare_baseline_and_scenario(baseline, &sim_reconstruction, &req.assumptions);

    let baseline_run_id = baseline
        .reconstruction_run_id
        .clone()
        .or_else(|| baseline.run_id.clone())
        .unwrap_or_else(|| "baseline-active".to_string());

    Ok(ScenarioSimulationResponse {
        baseline_run_id,
        scenario_run_id,
        scenario_name,
        is_hypothetical: true,
        simulated_at: Utc::now().to_rfc3339(),
        assumptions_applied: req.assumptions.clone(),
        comparison,
        simulated_reconstruction: sim_reconstruction,
    })
}

pub fn compare_baseline_and_scenario(
    baseline: &FullCaseReconstructionResult,
    scenario: &FullCaseReconstructionResult,
    assumptions: &ScenarioAssumptions,
) -> ScenarioComparisonResult {
    let mut findings_unchanged = Vec::new();
    let mut findings_newly_appearing = Vec::new();
    let mut findings_disappearing = Vec::new();
    let mut findings_robustness_changed = Vec::new();
    let mut explanations = Vec::new();

    // 1. Build summary of applied assumptions
    let mut applied_summary = Vec::new();
    if let Some(offset) = assumptions.default_clock_offset_seconds {
        applied_summary.push(format!("Default clock offset set to {:+.2}s", offset));
    }
    for (evi, off) in &assumptions.exhibit_clock_offsets {
        applied_summary.push(format!("Exhibit '{}' clock offset set to {:+.2}s", evi, off));
    }
    if let Some(ref tz) = assumptions.default_timezone {
        applied_summary.push(format!("Default timezone changed to '{}'", tz));
    }
    if let Some(speed) = assumptions.speed_threshold_kmh {
        applied_summary.push(format!("Speed threshold set to {:.1} km/h", speed));
    }
    if let Some(hw) = assumptions.estimated_half_width_seconds {
        applied_summary.push(format!("Estimated uncertainty half-width set to {:.1}s", hw));
    }
    if !assumptions.excluded_evidence_ids.is_empty() {
        applied_summary.push(format!(
            "Excluded {} evidence exhibit(s): [{}]",
            assumptions.excluded_evidence_ids.len(),
            assumptions.excluded_evidence_ids.join(", ")
        ));
    }
    if !assumptions.excluded_event_ids.is_empty() {
        applied_summary.push(format!(
            "Excluded {} event(s): [{}]",
            assumptions.excluded_event_ids.len(),
            assumptions.excluded_event_ids.join(", ")
        ));
    }
    for (ev_id, coord) in &assumptions.event_coordinate_overrides {
        applied_summary.push(format!(
            "Coordinate override on event '{}': ({:.4}, {:.4})",
            ev_id, coord.lat, coord.lon
        ));
    }
    for (ev_id, win) in &assumptions.event_uncertainty_overrides {
        applied_summary.push(format!(
            "Uncertainty window override on event '{}': [{}, {})",
            ev_id, win.start_utc, win.end_utc
        ));
    }

    // 2. Map baseline contradiction leads by signature
    type LeadKey = (ContradictionLeadType, String, String, String);
    let mut baseline_leads: BTreeMap<LeadKey, &ContradictionLead> = BTreeMap::new();
    for lead in &baseline.contradictions.leads {
        let key = (
            lead.lead_type,
            lead.entity_id.clone(),
            lead.event_a_id.clone(),
            lead.event_b_id.clone(),
        );
        baseline_leads.insert(key, lead);
    }

    let mut scenario_leads: BTreeMap<LeadKey, &ContradictionLead> = BTreeMap::new();
    for lead in &scenario.contradictions.leads {
        let key = (
            lead.lead_type,
            lead.entity_id.clone(),
            lead.event_a_id.clone(),
            lead.event_b_id.clone(),
        );
        scenario_leads.insert(key, lead);
    }

    // Compare scenario against baseline
    for (key, sc_lead) in &scenario_leads {
        if let Some(base_lead) = baseline_leads.get(key) {
            let lead_type_str = match sc_lead.lead_type {
                ContradictionLeadType::SpatialTransitImpossible => "SPATIAL_TRANSIT_IMPOSSIBLE",
                ContradictionLeadType::TemporalActivityInfeasible => "TEMPORAL_ACTIVITY_INFEASIBLE",
            };

            let base_speed = base_lead
                .derived_inference
                .post_activity_min_required_speed_kmh;
            let sc_speed = sc_lead
                .derived_inference
                .post_activity_min_required_speed_kmh;
            let base_elapsed = base_lead.derived_inference.min_possible_elapsed_seconds;
            let sc_elapsed = sc_lead.derived_inference.min_possible_elapsed_seconds;

            let mut robustness_changed = false;
            let mut metric_name = "metrics".to_string();
            let mut b_val = 0.0;
            let mut s_val = 0.0;
            let mut delta_expl = String::new();

            if let (Some(b_spd), Some(s_spd)) = (base_speed, sc_speed) {
                if (b_spd - s_spd).abs() > 0.1 {
                    robustness_changed = true;
                    metric_name = "min_required_speed_kmh".to_string();
                    b_val = b_spd;
                    s_val = s_spd;
                    delta_expl = format!(
                        "Minimum required transit speed for entity '{}' between events '{}' and '{}' shifted from {:.2} km/h (baseline) to {:.2} km/h (scenario).",
                        sc_lead.entity_id, sc_lead.event_a_id, sc_lead.event_b_id, b_spd, s_spd
                    );
                }
            } else if (base_elapsed - sc_elapsed).abs() > 0.1 {
                robustness_changed = true;
                metric_name = "min_possible_elapsed_seconds".to_string();
                b_val = base_elapsed;
                s_val = sc_elapsed;
                delta_expl = format!(
                    "Minimum possible elapsed seconds for entity '{}' between events '{}' and '{}' shifted from {:.1}s (baseline) to {:.1}s (scenario).",
                    sc_lead.entity_id, sc_lead.event_a_id, sc_lead.event_b_id, base_elapsed, sc_elapsed
                );
            }

            if robustness_changed {
                explanations.push(delta_expl.clone());
                findings_robustness_changed.push(FindingRobustnessDelta {
                    finding_id: sc_lead.lead_id.clone(),
                    finding_type: lead_type_str.to_string(),
                    entity_id: sc_lead.entity_id.clone(),
                    event_ids: vec![sc_lead.event_a_id.clone(), sc_lead.event_b_id.clone()],
                    metric_name,
                    baseline_value: b_val,
                    scenario_value: s_val,
                    baseline_metrics: serde_json::to_value(&base_lead.derived_inference)
                        .unwrap_or_default(),
                    scenario_metrics: serde_json::to_value(&sc_lead.derived_inference)
                        .unwrap_or_default(),
                    explanation: delta_expl,
                });
            } else {
                findings_unchanged.push(FindingComparisonItem {
                    finding_id: sc_lead.lead_id.clone(),
                    finding_type: lead_type_str.to_string(),
                    entity_id: sc_lead.entity_id.clone(),
                    event_ids: vec![sc_lead.event_a_id.clone(), sc_lead.event_b_id.clone()],
                    summary: sc_lead.neutral_explanation.clone(),
                    details: serde_json::to_value(sc_lead).unwrap_or_default(),
                });
            }
        } else {
            // Newly appearing finding
            let lead_type_str = match sc_lead.lead_type {
                ContradictionLeadType::SpatialTransitImpossible => "SPATIAL_TRANSIT_IMPOSSIBLE",
                ContradictionLeadType::TemporalActivityInfeasible => "TEMPORAL_ACTIVITY_INFEASIBLE",
            };
            let expl = format!(
                "Contradiction lead '{}' for entity '{}' between events '{}' and '{}' newly appeared under scenario assumptions: {}",
                lead_type_str, sc_lead.entity_id, sc_lead.event_a_id, sc_lead.event_b_id, sc_lead.neutral_explanation
            );
            explanations.push(expl);

            findings_newly_appearing.push(FindingComparisonItem {
                finding_id: sc_lead.lead_id.clone(),
                finding_type: lead_type_str.to_string(),
                entity_id: sc_lead.entity_id.clone(),
                event_ids: vec![sc_lead.event_a_id.clone(), sc_lead.event_b_id.clone()],
                summary: sc_lead.neutral_explanation.clone(),
                details: serde_json::to_value(sc_lead).unwrap_or_default(),
            });
        }
    }

    // Check baseline leads that disappeared in scenario
    for (key, base_lead) in &baseline_leads {
        if !scenario_leads.contains_key(key) {
            let lead_type_str = match base_lead.lead_type {
                ContradictionLeadType::SpatialTransitImpossible => "SPATIAL_TRANSIT_IMPOSSIBLE",
                ContradictionLeadType::TemporalActivityInfeasible => "TEMPORAL_ACTIVITY_INFEASIBLE",
            };

            let reason = if assumptions
                .excluded_evidence_ids
                .contains(&base_lead.observed_basis.origin_evidence_id)
                || assumptions
                    .excluded_evidence_ids
                    .contains(&base_lead.observed_basis.destination_evidence_id)
            {
                "one or more contributing evidence exhibits were excluded in this scenario"
            } else if assumptions
                .excluded_event_ids
                .contains(&base_lead.event_a_id)
                || assumptions
                    .excluded_event_ids
                    .contains(&base_lead.event_b_id)
            {
                "one or more contributing events were excluded in this scenario"
            } else if assumptions.speed_threshold_kmh.is_some() {
                "the relaxed speed threshold rendered transit feasible"
            } else if assumptions.default_clock_offset_seconds.is_some()
                || !assumptions.exhibit_clock_offsets.is_empty()
            {
                "clock drift adjustment expanded the transit time window sufficiently to make travel feasible"
            } else {
                "alternative temporal or spatial uncertainty bounds resolved the apparent contradiction"
            };

            let expl = format!(
                "Contradiction lead '{}' for entity '{}' between events '{}' and '{}' resolved/disappeared in this scenario because {}.",
                lead_type_str, base_lead.entity_id, base_lead.event_a_id, base_lead.event_b_id, reason
            );
            explanations.push(expl);

            findings_disappearing.push(FindingComparisonItem {
                finding_id: base_lead.lead_id.clone(),
                finding_type: lead_type_str.to_string(),
                entity_id: base_lead.entity_id.clone(),
                event_ids: vec![base_lead.event_a_id.clone(), base_lead.event_b_id.clone()],
                summary: format!("Disappeared: {}", base_lead.neutral_explanation),
                details: serde_json::to_value(base_lead).unwrap_or_default(),
            });
        }
    }

    // Trajectory count delta explanation
    let base_trajs = baseline.spatial.trajectories.len();
    let sc_trajs = scenario.spatial.trajectories.len();
    if base_trajs != sc_trajs {
        explanations.push(format!(
            "Number of active spatial trajectories changed from {} (baseline) to {} (scenario).",
            base_trajs, sc_trajs
        ));
    }

    ScenarioComparisonResult {
        findings_unchanged,
        findings_newly_appearing,
        findings_disappearing,
        findings_robustness_changed,
        applied_assumptions_summary: applied_summary,
        explanations,
    }
}
