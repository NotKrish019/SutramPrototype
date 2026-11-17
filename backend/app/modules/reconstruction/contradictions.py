"""Contradiction Detection Engine (Member 4 — Forensic Intelligence & System Integration).

Identifies conservative, evidence-backed temporal and spatial-transit discrepancies
across reconstructed timelines and entity trajectories.

Core forensic principles enforced:
1. Existential feasibility over uncertainty intervals: For event A in [A_start, A_end)
   with optional duration D and strictly ordered event B in [B_start, B_end), a lead is
   generated ONLY when NO valid pair (t_A, t_B) in [A_start, A_end) x [B_start, B_end)
   permits A to complete (t_A + D <= t_B) and transit to B within the reference speed
   threshold. Never generates a lead merely because nominal or worst-case bounds fail.
2. Overlapping uncertainty windows [start, end) are never treated as proof of contradiction.
3. Leads are never generated when chronological ordering between events is unresolved.
4. Speed threshold exceedance is reported strictly as a configurable reference lead,
   never as proof of physical impossibility, guilt, intent, or statistical probability.
5. Observed evidence facts are separated explicitly from derived analytical inferences.
6. Original Event, Evidence, and Relationship inputs are never mutated.
"""

from datetime import timedelta
from typing import Any, Mapping, Optional, Sequence

from pydantic import BaseModel

from app.modules.reconstruction.spatial import (
    DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
    _validate_distance_km,
    _validate_speed_threshold,
    reconstruct_spatial_trajectories,
    validate_geopoint,
)
from app.modules.reconstruction.temporal import (
    DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
    _validate_estimated_half_width,
    intervals_overlap,
)
from app.schemas.reconstruction import (
    ContradictionAnalysisResult,
    ContradictionLead,
    ContradictionLeadType,
    DerivedContradictionRelationship,
    DerivedLeadInference,
    MovementSegment,
    NormalizedTimeWindow,
    ObservedLeadBasis,
    SpatialReconstructionResult,
    SpatialTemporalOverlap,
    SpeedThresholdEvaluation,
    TemporalOrderingStatus,
    TimelineEntry,
    extract_field,
)


def _validate_evidence_provenance(
    events: Sequence[Mapping[str, Any] | BaseModel | Any],
    evidence_by_id: Optional[Mapping[str, Mapping[str, Any] | BaseModel | Any]],
) -> None:
    """Verify that every event carries a non-empty source_evidence_id and, when an
    evidence_by_id registry is supplied, that each referenced source_evidence_id resolves.
    """
    for idx, event in enumerate(events):
        raw_event_id = extract_field(event, "event_id", f"<index_{idx}>")
        raw_ev_id = extract_field(event, "source_evidence_id", None)
        if not isinstance(raw_ev_id, str) or not raw_ev_id.strip():
            raise ValueError(
                f"Event '{raw_event_id}' is missing required 'source_evidence_id' provenance."
            )
        clean_ev_id = raw_ev_id.strip()
        if evidence_by_id is not None:
            if not isinstance(evidence_by_id, Mapping):
                raise ValueError("Invalid evidence_by_id: expected a Mapping keyed by evidence_id.")
            if clean_ev_id not in evidence_by_id or evidence_by_id[clean_ev_id] is None:
                raise ValueError(
                    f"Event '{raw_event_id}' references source_evidence_id '{clean_ev_id}' "
                    "which is missing from the provided evidence_by_id provenance registry."
                )


def _build_limitations_list(
    *,
    entity_id: str,
    has_unlocated_intervening_events: bool = False,
    intervening_unlocated_event_ids: Optional[Sequence[str]] = None,
) -> list[str]:
    """Construct neutral analytical caveats without accusation or probability language."""
    limitations: list[str] = [
        (
            "The configured speed threshold is a user-adjustable reference benchmark for "
            "investigative triage and does not establish a physical upper bound on transit."
        ),
        (
            "This indicator is a derived analytical lead for human investigator review and "
            "does not establish factual inaccuracy, actor state of mind, or statistical likelihood."
        ),
        (
            "Alternative explanations include uncalibrated device clock offsets, shared or "
            "transferred device possession, high-speed transport modes, or coarse cell-tower "
            "location centroids."
        ),
    ]
    if has_unlocated_intervening_events and intervening_unlocated_event_ids:
        intervening_str = ", ".join(intervening_unlocated_event_ids)
        limitations.append(
            f"Incomplete spatial continuity: {len(intervening_unlocated_event_ids)} "
            f"intervening event(s) ({intervening_str}) for entity '{entity_id}' "
            "lacked geographic coordinates between origin and destination."
        )
    return limitations


def check_temporal_activity_feasibility(
    origin_time_window: NormalizedTimeWindow,
    destination_time_window: NormalizedTimeWindow,
    distance_km: Optional[float] = None,
    *,
    speed_threshold_kmh: float = DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
) -> tuple[bool, Optional[float], Optional[float], Optional[float]]:
    """Determine whether there exists ANY valid pair (t_A, t_B) in [A_start, A_end) x [B_start, B_end)
    that permits:
      1. A to begin within [A_start, A_end)
      2. A to complete after duration D (where D = origin_time_window.duration_seconds)
      3. B to occur strictly after A completes (or at/after A completes within [B_start, B_end))
      4. Travel over distance_km (when known and > 0) to occur within the available time
         at or below speed_threshold_kmh.

    Half-open interval [start, end) semantics (supremum vs. attained maximum):
    - Because t_A in [A_start, A_end) attains its minimum at t_A = A_start, while t_B in
      [B_start, B_end) is strictly bounded above by the exclusive upper bound B_end (t_B < B_end),
      post_max_elapsed = (B_end - (A_start + D)).total_seconds() is a strict mathematical
      supremum (never attained at t_B = B_end).
    - Every valid pair (t_A, t_B) has attained post-activity time strictly less than post_max_elapsed.
    - Consequently:
      * For d is None or d == 0.0: a valid (t_A, t_B) with t_B > t_A + D (and t_B >= t_A + D)
        exists in [A_start, A_end) x [B_start, B_end) iff A_start + D < B_end (post_max_elapsed > 0.0).
      * For d > 0.0: a valid (t_A, t_B) with attained speed <= speed_threshold_kmh exists in
        [A_start, A_end) x [B_start, B_end) iff A_start + D + t_travel_min < B_end, which is
        equivalent to post_max_elapsed > t_travel_min (or infimum speed post_min_speed < speed_threshold_kmh).

    Returns:
      (is_feasible, post_activity_min_elapsed_seconds, post_activity_max_elapsed_seconds, post_activity_min_speed_kmh)
    """
    if not isinstance(origin_time_window, NormalizedTimeWindow) or not isinstance(
        destination_time_window, NormalizedTimeWindow
    ):
        raise ValueError(
            "origin_time_window and destination_time_window must be NormalizedTimeWindow instances."
        )
    if (
        origin_time_window.window_start_utc >= origin_time_window.window_end_utc
        or destination_time_window.window_start_utc >= destination_time_window.window_end_utc
    ):
        raise ValueError(
            "Invalid half-open interval [start, end): window_start_utc must be strictly less than window_end_utc."
        )

    threshold = _validate_speed_threshold(speed_threshold_kmh)
    dist = _validate_distance_km(distance_km) if distance_km is not None else None

    duration = origin_time_window.duration_seconds
    if duration is None:
        return True, None, None, None

    earliest_completion_utc = origin_time_window.window_start_utc + timedelta(seconds=duration)
    latest_completion_utc = origin_time_window.window_end_utc + timedelta(seconds=duration)

    # Supremum (unattained least upper bound) of available time after A completes before B occurs:
    # approached as t_A = A_start and t_B -> B_end^-.
    post_max_elapsed = (
        destination_time_window.window_end_utc - earliest_completion_utc
    ).total_seconds()

    # Infimum (worst-case lower bound) of available time after A completes before B occurs:
    # approached as t_A -> A_end^- and t_B = B_start.
    post_min_elapsed = (
        destination_time_window.window_start_utc - latest_completion_utc
    ).total_seconds()

    # Regime 1: Coordinates unknown (dist is None) or zero-distance stationary (dist == 0.0)
    if dist is None or dist == 0.0:
        # Because [B_start, B_end) is half-open (t_B < B_end) and [A_start, A_end) has
        # inclusive lower bound (t_A >= A_start), there exists (t_A, t_B) with t_A + D < t_B
        # (and t_A + D <= t_B) if and only if A_start + D < B_end (i.e., post_max_elapsed > 0.0).
        is_feasible = post_max_elapsed > 0.0
        post_min_speed = 0.0 if (dist == 0.0 and is_feasible) else None
        return is_feasible, post_min_elapsed, post_max_elapsed, post_min_speed

    # Regime 2: Known positive distance (dist > 0.0)
    if post_max_elapsed <= 0.0:
        # Positive distance cannot be traversed in zero or negative available time.
        return False, post_min_elapsed, post_max_elapsed, None

    # Infimum of required speed over [A_start, A_end) x [B_start, B_end).
    # Because post_max_elapsed is an unattained supremum (t_B < B_end), every attained timestamp
    # pair (t_A, t_B) requires speed strictly greater than post_min_speed. Therefore, an attained
    # speed <= threshold exists iff post_min_speed < threshold.
    post_min_speed = (dist * 3600.0) / post_max_elapsed
    is_feasible = post_min_speed < threshold
    return is_feasible, post_min_elapsed, post_max_elapsed, post_min_speed


def evaluate_segment_for_contradiction(
    segment: MovementSegment,
) -> Optional[ContradictionLead]:
    """Evaluate a strictly ordered MovementSegment for conservative spatial/temporal discrepancies.

    Returns None if:
    - Event ordering is not STRICTLY_ORDERED or windows overlap.
    - At least one valid timestamp pair (t_A, t_B) within the uncertainty windows satisfies
      both activity duration completion and reference travel speed feasibility.
    """
    if not isinstance(segment, MovementSegment):
        raise ValueError("Expected a MovementSegment instance.")

    if (
        not isinstance(segment.origin_source_evidence_id, str)
        or not segment.origin_source_evidence_id.strip()
        or not isinstance(segment.destination_source_evidence_id, str)
        or not segment.destination_source_evidence_id.strip()
    ):
        raise ValueError(
            f"Segment '{segment.segment_id}' is missing required source evidence provenance."
        )

    # Never generate a lead if event ordering is unresolved or windows intersect
    if segment.ordering_status != TemporalOrderingStatus.STRICTLY_ORDERED:
        return None

    win_a = segment.origin_time_window
    win_b = segment.destination_time_window
    if (
        intervals_overlap(
            win_a.window_start_utc,
            win_a.window_end_utc,
            win_b.window_start_utc,
            win_b.window_end_utc,
        )
        is not None
        or win_a.window_end_utc > win_b.window_start_utc
    ):
        return None

    (
        is_activity_feasible,
        post_activity_min_elapsed,
        post_activity_max_elapsed,
        post_activity_min_speed,
    ) = check_temporal_activity_feasibility(
        win_a,
        win_b,
        distance_km=segment.distance_km,
        speed_threshold_kmh=segment.configured_speed_threshold_kmh,
    )
    has_activity_overlap_conflict = not is_activity_feasible

    has_speed_exceedance = (
        segment.distance_km > 0.0
        and segment.exceeds_configured_speed_threshold
        and segment.threshold_evaluation
        in (
            SpeedThresholdEvaluation.EXCEEDS_CONFIGURED_THRESHOLD,
            SpeedThresholdEvaluation.NON_POSITIVE_ELAPSED_TIME,
        )
    )

    if not has_activity_overlap_conflict and not has_speed_exceedance:
        return None

    lead_type = (
        ContradictionLeadType.TEMPORAL_ACTIVITY_OVERLAP_LEAD
        if has_activity_overlap_conflict and not has_speed_exceedance
        else ContradictionLeadType.SPEED_THRESHOLD_EXCEEDANCE_LEAD
    )

    lead_id = (
        f"lead-{segment.case_id}-{segment.entity_id}-"
        f"{segment.origin_event_id}-to-{segment.destination_event_id}"
    )
    relationship_id = (
        f"rel-contra-{segment.case_id}-{segment.entity_id}-"
        f"{segment.origin_event_id}-to-{segment.destination_event_id}"
    )

    evidence_basis = list(
        dict.fromkeys(
            [
                segment.origin_source_evidence_id.strip(),
                segment.destination_source_evidence_id.strip(),
            ]
        )
    )

    observed_facts = ObservedLeadBasis(
        entity_id=segment.entity_id,
        origin_event_id=segment.origin_event_id,
        destination_event_id=segment.destination_event_id,
        origin_source_evidence_id=segment.origin_source_evidence_id,
        destination_source_evidence_id=segment.destination_source_evidence_id,
        origin_location_name=segment.origin_location_name,
        destination_location_name=segment.destination_location_name,
        origin_coordinates=segment.origin_coordinates,
        destination_coordinates=segment.destination_coordinates,
        origin_time_window=win_a,
        destination_time_window=win_b,
        origin_duration_seconds=win_a.duration_seconds,
    )

    derived_inference = DerivedLeadInference(
        is_observed=False,
        ordering_status=TemporalOrderingStatus.STRICTLY_ORDERED,
        distance_km=segment.distance_km,
        nominal_elapsed_seconds=segment.nominal_elapsed_seconds,
        min_possible_elapsed_seconds=segment.min_possible_elapsed_seconds,
        max_possible_elapsed_seconds=segment.max_possible_elapsed_seconds,
        post_activity_min_elapsed_seconds=post_activity_min_elapsed,
        post_activity_max_elapsed_seconds=post_activity_max_elapsed,
        min_required_speed_kmh=segment.min_required_speed_kmh,
        post_activity_min_required_speed_kmh=post_activity_min_speed,
        configured_speed_threshold_kmh=segment.configured_speed_threshold_kmh,
        threshold_evaluation=segment.threshold_evaluation,
        intervening_unlocated_event_ids=list(segment.intervening_unlocated_event_ids),
        has_unlocated_intervening_events=segment.has_unlocated_intervening_events,
    )

    origin_loc_label = segment.origin_location_name or (
        f"({segment.origin_coordinates.lat:.4f}, {segment.origin_coordinates.lon:.4f})"
    )
    dest_loc_label = segment.destination_location_name or (
        f"({segment.destination_coordinates.lat:.4f}, {segment.destination_coordinates.lon:.4f})"
    )

    origin_duration = win_a.duration_seconds
    if lead_type == ContradictionLeadType.TEMPORAL_ACTIVITY_OVERLAP_LEAD:
        if post_activity_max_elapsed is not None and post_activity_max_elapsed <= 0.0:
            explanation = (
                f"Observed event '{segment.origin_event_id}' (evidence '{segment.origin_source_evidence_id}') "
                f"for entity '{segment.entity_id}' at {origin_loc_label} has uncertainty window "
                f"[{win_a.window_start_utc.isoformat()}, {win_a.window_end_utc.isoformat()}) and explicit "
                f"duration {origin_duration:.1f}s, leaving {post_activity_max_elapsed:.1f}s of maximum "
                f"available post-activity time before the upper window bound of subsequent event "
                f"'{segment.destination_event_id}' (evidence '{segment.destination_source_evidence_id}') "
                f"at {dest_loc_label} ([{win_b.window_start_utc.isoformat()}, {win_b.window_end_utc.isoformat()}), "
                f"distance {segment.distance_km:.2f} km)."
            )
        else:
            explanation = (
                f"Observed event '{segment.origin_event_id}' (evidence '{segment.origin_source_evidence_id}') "
                f"for entity '{segment.entity_id}' at {origin_loc_label} with duration {origin_duration:.1f}s "
                f"leaves at most {post_activity_max_elapsed:.1f}s before subsequent event "
                f"'{segment.destination_event_id}' (evidence '{segment.destination_source_evidence_id}') "
                f"at {dest_loc_label} ({segment.distance_km:.2f} km apart), requiring a minimum transit "
                f"speed of {post_activity_min_speed:.2f} km/h compared against the configured reference "
                f"threshold of {segment.configured_speed_threshold_kmh:.2f} km/h."
            )
    else:
        speed_str = (
            f"{segment.min_required_speed_kmh:.2f} km/h"
            if segment.min_required_speed_kmh is not None
            else "undefined (non-positive elapsed window)"
        )
        explanation = (
            f"Observed event '{segment.origin_event_id}' (evidence '{segment.origin_source_evidence_id}') "
            f"at {origin_loc_label} ([{win_a.window_start_utc.isoformat()}, {win_a.window_end_utc.isoformat()})) "
            f"and subsequent strictly ordered event '{segment.destination_event_id}' "
            f"(evidence '{segment.destination_source_evidence_id}') at {dest_loc_label} "
            f"([{win_b.window_start_utc.isoformat()}, {win_b.window_end_utc.isoformat()})) for entity "
            f"'{segment.entity_id}' are separated by {segment.distance_km:.2f} km over a conservative "
            f"maximum elapsed window of {segment.max_possible_elapsed_seconds:.1f}s, requiring at least "
            f"{speed_str} compared against the configured reference threshold of "
            f"{segment.configured_speed_threshold_kmh:.2f} km/h."
        )

    limitations = _build_limitations_list(
        entity_id=segment.entity_id,
        has_unlocated_intervening_events=segment.has_unlocated_intervening_events,
        intervening_unlocated_event_ids=segment.intervening_unlocated_event_ids,
    )

    relationship = DerivedContradictionRelationship(
        relationship_id=relationship_id,
        source_id=segment.origin_event_id,
        target_id=segment.destination_event_id,
        relation_type="CONTRADICTS",
        weight=None,
        is_observed=False,
        evidence_basis=evidence_basis,
        metadata={
            "lead_id": lead_id,
            "lead_type": lead_type.value,
            "case_id": segment.case_id,
            "entity_id": segment.entity_id,
            "distance_km": segment.distance_km,
            "max_possible_elapsed_seconds": segment.max_possible_elapsed_seconds,
            "post_activity_max_elapsed_seconds": post_activity_max_elapsed,
            "min_required_speed_kmh": segment.min_required_speed_kmh,
            "post_activity_min_required_speed_kmh": post_activity_min_speed,
            "configured_speed_threshold_kmh": segment.configured_speed_threshold_kmh,
            "has_unlocated_intervening_events": segment.has_unlocated_intervening_events,
            "intervening_unlocated_event_ids": list(segment.intervening_unlocated_event_ids),
            "explanation": explanation,
            "limitations": limitations,
        },
    )

    return ContradictionLead(
        lead_id=lead_id,
        case_id=segment.case_id,
        entity_id=segment.entity_id,
        lead_type=lead_type,
        origin_event_id=segment.origin_event_id,
        destination_event_id=segment.destination_event_id,
        evidence_basis=evidence_basis,
        observed_facts=observed_facts,
        derived_inference=derived_inference,
        explanation=explanation,
        limitations=limitations,
        relationship=relationship,
    )


def _evaluate_unlocated_pair_for_activity_conflict(
    origin_entry: TimelineEntry,
    destination_entry: TimelineEntry,
    entity_id: str,
    *,
    speed_threshold_kmh: float = DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
) -> Optional[ContradictionLead]:
    """Evaluate two strictly ordered TimelineEntries where at least one lacks coordinates
    to detect pure temporal activity-duration incompatibility (A_start + D > B_end).
    """
    win_a = origin_entry.time_window
    win_b = destination_entry.time_window
    if win_a.duration_seconds is None:
        return None

    if (
        intervals_overlap(
            win_a.window_start_utc,
            win_a.window_end_utc,
            win_b.window_start_utc,
            win_b.window_end_utc,
        )
        is not None
        or win_a.window_end_utc > win_b.window_start_utc
    ):
        return None

    (
        is_feasible,
        post_min_elapsed,
        post_max_elapsed,
        post_min_speed,
    ) = check_temporal_activity_feasibility(
        win_a,
        win_b,
        distance_km=None,
        speed_threshold_kmh=speed_threshold_kmh,
    )
    if is_feasible:
        return None

    origin_coords = validate_geopoint(origin_entry.coordinates)
    dest_coords = validate_geopoint(destination_entry.coordinates)

    nominal_elapsed = (win_b.nominal_utc - win_a.nominal_utc).total_seconds()
    min_elapsed = (win_b.window_start_utc - win_a.window_end_utc).total_seconds()
    max_elapsed = (win_b.window_end_utc - win_a.window_start_utc).total_seconds()

    lead_id = (
        f"lead-{origin_entry.case_id}-{entity_id}-"
        f"{origin_entry.event_id}-to-{destination_entry.event_id}"
    )
    relationship_id = (
        f"rel-contra-{origin_entry.case_id}-{entity_id}-"
        f"{origin_entry.event_id}-to-{destination_entry.event_id}"
    )
    evidence_basis = list(
        dict.fromkeys(
            [
                origin_entry.source_evidence_id.strip(),
                destination_entry.source_evidence_id.strip(),
            ]
        )
    )

    observed_facts = ObservedLeadBasis(
        entity_id=entity_id,
        origin_event_id=origin_entry.event_id,
        destination_event_id=destination_entry.event_id,
        origin_source_evidence_id=origin_entry.source_evidence_id,
        destination_source_evidence_id=destination_entry.source_evidence_id,
        origin_location_name=origin_entry.location_name,
        destination_location_name=destination_entry.location_name,
        origin_coordinates=origin_coords,
        destination_coordinates=dest_coords,
        origin_time_window=win_a,
        destination_time_window=win_b,
        origin_duration_seconds=win_a.duration_seconds,
    )

    derived_inference = DerivedLeadInference(
        is_observed=False,
        ordering_status=TemporalOrderingStatus.STRICTLY_ORDERED,
        distance_km=None,
        nominal_elapsed_seconds=nominal_elapsed,
        min_possible_elapsed_seconds=min_elapsed,
        max_possible_elapsed_seconds=max_elapsed,
        post_activity_min_elapsed_seconds=post_min_elapsed,
        post_activity_max_elapsed_seconds=post_max_elapsed,
        min_required_speed_kmh=None,
        post_activity_min_required_speed_kmh=post_min_speed,
        configured_speed_threshold_kmh=speed_threshold_kmh,
        threshold_evaluation=SpeedThresholdEvaluation.NON_POSITIVE_ELAPSED_TIME,
        intervening_unlocated_event_ids=[],
        has_unlocated_intervening_events=False,
    )

    explanation = (
        f"Observed event '{origin_entry.event_id}' (evidence '{origin_entry.source_evidence_id}') "
        f"for entity '{entity_id}' has start uncertainty window "
        f"[{win_a.window_start_utc.isoformat()}, {win_a.window_end_utc.isoformat()}) and explicit "
        f"duration {win_a.duration_seconds:.1f}s, which extends beyond the entire uncertainty window "
        f"[{win_b.window_start_utc.isoformat()}, {win_b.window_end_utc.isoformat()}) of subsequent "
        f"event '{destination_entry.event_id}' (evidence '{destination_entry.source_evidence_id}', "
        f"post-activity maximum available time {post_max_elapsed:.1f}s)."
    )
    limitations = _build_limitations_list(entity_id=entity_id)

    relationship = DerivedContradictionRelationship(
        relationship_id=relationship_id,
        source_id=origin_entry.event_id,
        target_id=destination_entry.event_id,
        relation_type="CONTRADICTS",
        weight=None,
        is_observed=False,
        evidence_basis=evidence_basis,
        metadata={
            "lead_id": lead_id,
            "lead_type": ContradictionLeadType.TEMPORAL_ACTIVITY_OVERLAP_LEAD.value,
            "case_id": origin_entry.case_id,
            "entity_id": entity_id,
            "distance_km": None,
            "max_possible_elapsed_seconds": max_elapsed,
            "post_activity_max_elapsed_seconds": post_max_elapsed,
            "configured_speed_threshold_kmh": speed_threshold_kmh,
            "explanation": explanation,
            "limitations": limitations,
        },
    )

    return ContradictionLead(
        lead_id=lead_id,
        case_id=origin_entry.case_id,
        entity_id=entity_id,
        lead_type=ContradictionLeadType.TEMPORAL_ACTIVITY_OVERLAP_LEAD,
        origin_event_id=origin_entry.event_id,
        destination_event_id=destination_entry.event_id,
        evidence_basis=evidence_basis,
        observed_facts=observed_facts,
        derived_inference=derived_inference,
        explanation=explanation,
        limitations=limitations,
        relationship=relationship,
    )


def detect_contradictions_from_spatial_result(
    spatial_result: SpatialReconstructionResult,
) -> ContradictionAnalysisResult:
    """Analyze an existing SpatialReconstructionResult and return deduplicated ContradictionLeads."""
    if not isinstance(spatial_result, SpatialReconstructionResult):
        raise ValueError("Expected a SpatialReconstructionResult instance.")

    leads: list[ContradictionLead] = []
    seen_lead_keys: set[tuple[str, str, str]] = set()
    unresolved_overlaps: list[SpatialTemporalOverlap] = []

    for entity_id in sorted(spatial_result.trajectories_by_entity.keys()):
        trajectory = spatial_result.trajectories_by_entity[entity_id]

        # Record overlapping located pairs in unresolved_overlaps_skipped without generating leads
        for overlap in trajectory.uncertain_spatial_overlaps:
            unresolved_overlaps.append(overlap)

        # 1. Evaluate located strictly-ordered movement segments
        for segment in trajectory.segments:
            dedup_key = (
                segment.entity_id,
                segment.origin_event_id,
                segment.destination_event_id,
            )
            if dedup_key in seen_lead_keys:
                continue

            lead = evaluate_segment_for_contradiction(segment)
            if lead is not None:
                seen_lead_keys.add(dedup_key)
                leads.append(lead)

        # 2. Evaluate strictly-ordered consecutive timeline entries where one or both lack coordinates
        # for pure temporal activity-duration exceedance (A_start + D > B_end).
        entries = trajectory.timeline_entries
        entity_entry_ids = {e.event_id for e in entries}
        for k in range(len(entries) - 1):
            entry_a = entries[k]
            entry_b = entries[k + 1]
            dedup_key = (entity_id, entry_a.event_id, entry_b.event_id)
            if dedup_key in seen_lead_keys:
                continue

            # Skip if either entry is involved in an unresolved temporal overlap for this entity
            if (
                set(entry_a.concurrent_event_ids) & entity_entry_ids
                or set(entry_b.concurrent_event_ids) & entity_entry_ids
            ):
                continue

            if entry_a.coordinates is None or entry_b.coordinates is None:
                unloc_lead = _evaluate_unlocated_pair_for_activity_conflict(
                    entry_a,
                    entry_b,
                    entity_id,
                    speed_threshold_kmh=spatial_result.configured_speed_threshold_kmh,
                )
                if unloc_lead is not None:
                    seen_lead_keys.add(dedup_key)
                    leads.append(unloc_lead)

    # Sort leads deterministically by (entity_id, origin window start, destination window start, lead_id)
    leads.sort(
        key=lambda item: (
            item.entity_id,
            item.observed_facts.origin_time_window.window_start_utc,
            item.observed_facts.destination_time_window.window_start_utc,
            item.lead_id,
        )
    )

    relationships = [lead.relationship for lead in leads]

    return ContradictionAnalysisResult(
        case_id=spatial_result.case_id,
        configured_speed_threshold_kmh=spatial_result.configured_speed_threshold_kmh,
        leads=leads,
        relationships=relationships,
        unresolved_overlaps_skipped=unresolved_overlaps,
    )


def detect_contradictions(
    events: Sequence[Mapping[str, Any] | BaseModel | Any],
    evidence_by_id: Optional[Mapping[str, Mapping[str, Any] | BaseModel | Any]] = None,
    *,
    case_id: Optional[str] = None,
    speed_threshold_kmh: float = DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
    estimated_half_width_seconds: float = DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
) -> ContradictionAnalysisResult:
    """Run end-to-end temporal, spatial, and conservative contradiction lead detection.

    - Validates event and evidence provenance without mutating inputs.
    - Preserves uncertainty windows and skips pairs with unresolved event ordering.
    - Emits deterministic ContradictionLead and DerivedContradictionRelationship records
      (relation_type='CONTRADICTS', is_observed=False).
    """
    if not isinstance(events, Sequence) or isinstance(events, (str, bytes)):
        raise ValueError("Invalid events input: expected a sequence of Event records.")

    threshold = _validate_speed_threshold(speed_threshold_kmh)
    half_width = _validate_estimated_half_width(estimated_half_width_seconds)

    _validate_evidence_provenance(events, evidence_by_id)

    spatial_result = reconstruct_spatial_trajectories(
        events,
        evidence_by_id=evidence_by_id,
        case_id=case_id,
        speed_threshold_kmh=threshold,
        estimated_half_width_seconds=half_width,
    )

    return detect_contradictions_from_spatial_result(spatial_result)
