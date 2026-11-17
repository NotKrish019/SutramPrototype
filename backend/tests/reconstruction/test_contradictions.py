"""Unit tests for the Member 4 Phase 2C Contradiction Detection Engine.

All tests use fixed, deterministic timestamps and coordinates.
"""

from datetime import datetime, timezone
from copy import deepcopy
import itertools

import pytest

from app.modules.reconstruction.contradictions import (
    check_temporal_activity_feasibility,
    detect_contradictions,
    detect_contradictions_from_spatial_result,
    evaluate_segment_for_contradiction,
)
from app.modules.reconstruction.spatial import reconstruct_spatial_trajectories
from app.modules.reconstruction.temporal import normalize_event_time
from app.schemas.reconstruction import (
    CanonicalTimePrecision,
    ContradictionLeadType,
    NormalizedTimeWindow,
    SpeedThresholdEvaluation,
    TemporalOrderingStatus,
)


class TestContradictionDetectionRules:
    """Tests for conservative spatial/travel and temporal activity-overlap lead detection."""

    def test_genuine_speed_threshold_exceedance_lead_and_relationship_contract(self) -> None:
        # New Delhi (10:00:00Z) to Mumbai (10:10:00Z): ~1148 km in 601s max elapsed window
        events = [
            {
                "event_id": "evt-delhi",
                "case_id": "case-2c",
                "title": "CCTV Sighting at New Delhi",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "New Delhi Connaught Place",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-suspect-1"],
                "source_evidence_id": "ev-cctv-delhi",
            },
            {
                "event_id": "evt-mumbai",
                "case_id": "case-2c",
                "title": "CDR Tower Attachment at Mumbai",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Mumbai Nariman Point",
                "coordinates": {"lat": 19.0760, "lon": 72.8777},
                "involved_entity_ids": ["ent-suspect-1"],
                "source_evidence_id": "ev-cdr-mumbai",
            },
        ]

        result = detect_contradictions(events, speed_threshold_kmh=180.0)
        assert result.case_id == "case-2c"
        assert len(result.leads) == 1
        assert len(result.relationships) == 1

        lead = result.leads[0]
        assert lead.lead_id == "lead-case-2c-ent-suspect-1-evt-delhi-to-evt-mumbai"
        assert lead.lead_type == ContradictionLeadType.SPEED_THRESHOLD_EXCEEDANCE_LEAD
        assert lead.entity_id == "ent-suspect-1"
        assert lead.origin_event_id == "evt-delhi"
        assert lead.destination_event_id == "evt-mumbai"
        assert lead.evidence_basis == ["ev-cctv-delhi", "ev-cdr-mumbai"]

        # Verify separation of observed facts vs derived inference
        assert lead.observed_facts.origin_source_evidence_id == "ev-cctv-delhi"
        assert lead.observed_facts.destination_source_evidence_id == "ev-cdr-mumbai"
        assert lead.observed_facts.origin_location_name == "New Delhi Connaught Place"
        assert lead.observed_facts.destination_location_name == "Mumbai Nariman Point"
        assert lead.derived_inference.is_observed is False
        assert lead.derived_inference.distance_km > 1140.0
        assert lead.derived_inference.max_possible_elapsed_seconds == 601.0
        assert (lead.derived_inference.min_required_speed_kmh or 0.0) > 6800.0
        assert (
            lead.derived_inference.threshold_evaluation
            == SpeedThresholdEvaluation.EXCEEDS_CONFIGURED_THRESHOLD
        )

        # Verify canonical Relationship contract compatibility
        rel = lead.relationship
        assert rel == result.relationships[0]
        assert rel.relationship_id == "rel-contra-case-2c-ent-suspect-1-evt-delhi-to-evt-mumbai"
        assert rel.source_id == "evt-delhi"
        assert rel.target_id == "evt-mumbai"
        assert rel.relation_type == "CONTRADICTS"
        assert rel.is_observed is False
        assert rel.weight is None
        assert rel.evidence_basis == ["ev-cctv-delhi", "ev-cdr-mumbai"]

    def test_temporal_activity_overlap_lead_with_explicit_duration(self) -> None:
        # Event A starts at 10:00:00Z with explicit duration_seconds = 3600.0 (until 11:00:00Z)
        # Event B starts at 10:45:00Z (strictly ordered start-time window after 10:00:01Z) at a
        # location 12 km away. Start-to-start speed is ~16 km/h (<= 180 km/h), but Event A's
        # 3600s duration is still active at 10:45:00Z!
        events = [
            {
                "event_id": "evt-interview",
                "case_id": "case-dur",
                "title": "Recorded In-Person Session",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 3600.0,
                "location_name": "Office Alpha",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-log-1",
            },
            {
                "event_id": "evt-atm",
                "case_id": "case-dur",
                "title": "Physical ATM Withdrawal",
                "timestamp": "2026-11-10T10:45:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "ATM Beta",
                "coordinates": {"lat": 28.5355, "lon": 77.3000},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-atm-2",
            },
        ]

        result = detect_contradictions(events, speed_threshold_kmh=180.0)
        assert len(result.leads) == 1
        lead = result.leads[0]
        assert lead.lead_type == ContradictionLeadType.TEMPORAL_ACTIVITY_OVERLAP_LEAD
        assert lead.observed_facts.origin_duration_seconds == 3600.0
        assert (lead.derived_inference.post_activity_max_elapsed_seconds or 0.0) < 0.0
        assert lead.relationship.relation_type == "CONTRADICTS"
        assert lead.relationship.is_observed is False

    def test_overlapping_uncertainty_windows_and_unresolved_ordering_never_generate_leads(
        self,
    ) -> None:
        # 1. Overlapping windows: evt-coarse [10:00Z, 11:00Z) and evt-exact [10:30:00Z, 10:30:01Z)
        # separated by ~1148 km (Delhi vs Mumbai). Because ordering is unresolved, NO contradiction
        # lead may be generated; the overlap is recorded in unresolved_overlaps_skipped.
        overlapping_events = [
            {
                "event_id": "evt-coarse-delhi",
                "case_id": "case-ov",
                "title": "Coarse Hour Log in Delhi",
                "timestamp": "2026-11-10T10:15:00Z",
                "timestamp_precision": "HOUR",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-exact-mumbai",
                "case_id": "case-ov",
                "title": "Exact Log in Mumbai Within Same Hour",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 19.0760, "lon": 72.8777},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]
        ov_result = detect_contradictions(overlapping_events)
        assert ov_result.leads == []
        assert ov_result.relationships == []
        assert len(ov_result.unresolved_overlaps_skipped) == 1
        assert (
            ov_result.unresolved_overlaps_skipped[0].ordering_status
            == TemporalOrderingStatus.CONCURRENT_UNCERTAIN
        )

        # 2. Strictly ordered but wide HOUR window [10:00Z, 11:00Z) followed by [11:01:00Z, 11:01:01Z)
        # ~20 km apart: conservative max elapsed time is 3661s (> 1h), so conservative min speed is
        # < 20 km/h (well below 180 km/h) -> no false-positive lead!
        wide_window_events = [
            {
                "event_id": "evt-hour-origin",
                "case_id": "case-wide",
                "title": "Hour-Precision Sighting",
                "timestamp": "2026-11-10T10:59:00Z",
                "timestamp_precision": "HOUR",  # [10:00Z, 11:00Z)
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-sec-dest",
                "case_id": "case-wide",
                "title": "Sighting 20km Away at 11:01Z",
                "timestamp": "2026-11-10T11:01:00Z",
                "timestamp_precision": "EXACT_SECOND",  # [11:01:00Z, 11:01:01Z)
                "coordinates": {"lat": 28.5355, "lon": 77.3910},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]
        wide_result = detect_contradictions(wide_window_events, speed_threshold_kmh=180.0)
        assert wide_result.leads == []

    def test_missing_coordinates_preserved_and_reported_in_limitations(self) -> None:
        events = [
            {
                "event_id": "evt-1",
                "case_id": "case-unloc",
                "title": "Delhi Sighting",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-2-unlocated",
                "case_id": "case-unloc",
                "title": "Unlocated Chat Message",
                "timestamp": "2026-11-10T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": None,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
            {
                "event_id": "evt-3",
                "case_id": "case-unloc",
                "title": "Mumbai Sighting",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 19.0760, "lon": 72.8777},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-3",
            },
        ]

        result = detect_contradictions(events)
        assert len(result.leads) == 1
        lead = result.leads[0]
        assert lead.derived_inference.has_unlocated_intervening_events is True
        assert lead.derived_inference.intervening_unlocated_event_ids == ["evt-2-unlocated"]
        assert any("evt-2-unlocated" in lim for lim in lead.limitations)

    def test_missing_evidence_provenance_rejected(self) -> None:
        # 1. Missing or blank source_evidence_id on event
        for bad_ev_id in (None, "", "   "):
            with pytest.raises(ValueError, match="source_evidence_id"):
                detect_contradictions(
                    [
                        {
                            "event_id": "evt-no-prov",
                            "case_id": "case-prov",
                            "title": "Missing Provenance Event",
                            "timestamp": "2026-11-10T10:00:00Z",
                            "timestamp_precision": "EXACT_SECOND",
                            "coordinates": {"lat": 28.6139, "lon": 77.2090},
                            "involved_entity_ids": ["ent-1"],
                            "source_evidence_id": bad_ev_id,
                        }
                    ]
                )

        # 2. Event references a source_evidence_id not present in provided evidence_by_id registry
        with pytest.raises(ValueError, match="ev-missing-99"):
            detect_contradictions(
                [
                    {
                        "event_id": "evt-orphan",
                        "case_id": "case-prov",
                        "title": "Orphan Provenance Event",
                        "timestamp": "2026-11-10T10:00:00Z",
                        "timestamp_precision": "EXACT_SECOND",
                        "coordinates": {"lat": 28.6139, "lon": 77.2090},
                        "involved_entity_ids": ["ent-1"],
                        "source_evidence_id": "ev-missing-99",
                    }
                ],
                evidence_by_id={"ev-known-1": {"metadata": {"source_timezone": "UTC"}}},
            )

    def test_multiple_entities_and_duplicate_detection_prevention(self) -> None:
        events = [
            {
                "event_id": "evt-1",
                "case_id": "case-multi",
                "title": "Delhi Start",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                # Duplicate entity ID in list should be deduplicated
                "involved_entity_ids": ["ent-fast", "ent-fast", "ent-slow"],
                "source_evidence_id": "ev-shared",
            },
            {
                "event_id": "evt-2-fast",
                "case_id": "case-multi",
                "title": "Mumbai 10m Later (ent-fast)",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 19.0760, "lon": 72.8777},
                "involved_entity_ids": ["ent-fast"],
                "source_evidence_id": "ev-shared",
            },
            {
                "event_id": "evt-2-slow",
                "case_id": "case-multi",
                "title": "Nearby 1km 30m Later (ent-slow)",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 28.6200, "lon": 77.2150},
                "involved_entity_ids": ["ent-slow"],
                "source_evidence_id": "ev-other",
            },
        ]

        result = detect_contradictions(events, speed_threshold_kmh=180.0)
        assert len(result.leads) == 1
        lead = result.leads[0]
        assert lead.entity_id == "ent-fast"
        # Same source_evidence_id on both events is deduplicated in evidence_basis
        assert lead.evidence_basis == ["ev-shared"]

    def test_deterministic_output_across_input_permutations_and_immutability(self) -> None:
        raw_events = [
            {
                "event_id": "evt-3",
                "case_id": "case-det",
                "title": "Bangalore Sighting",
                "timestamp": "2026-11-10T10:20:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 12.9716, "lon": 77.5946},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-3",
            },
            {
                "event_id": "evt-1",
                "case_id": "case-det",
                "title": "Delhi Sighting",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-2",
                "case_id": "case-det",
                "title": "Mumbai Sighting",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 19.0760, "lon": 72.8777},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]
        raw_evidence = {
            "ev-1": {"metadata": {"source_timezone": "UTC", "clock_offset_seconds": 0.0}},
            "ev-2": {"metadata": {"source_timezone": "UTC", "clock_offset_seconds": 0.0}},
            "ev-3": {"metadata": {"source_timezone": "UTC", "clock_offset_seconds": 0.0}},
        }

        events_snapshot = deepcopy(raw_events)
        evidence_snapshot = deepcopy(raw_evidence)

        baseline = detect_contradictions(raw_events, evidence_by_id=raw_evidence)
        assert len(baseline.leads) == 2

        # Verify inputs were never mutated
        assert raw_events == events_snapshot
        assert raw_evidence == evidence_snapshot

        for perm in itertools.permutations(raw_events):
            perm_result = detect_contradictions(list(perm), evidence_by_id=raw_evidence)
            assert perm_result == baseline

    def test_no_unsupported_accusation_or_probability_language(self) -> None:
        events = [
            {
                "event_id": "evt-a",
                "case_id": "case-lang",
                "title": "Sighting A",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Delhi",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-a",
            },
            {
                "event_id": "evt-b",
                "case_id": "case-lang",
                "title": "Sighting B",
                "timestamp": "2026-11-10T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Mumbai",
                "coordinates": {"lat": 19.0760, "lon": 72.8777},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-b",
            },
        ]

        result = detect_contradictions(events)
        assert len(result.leads) == 1
        lead = result.leads[0]

        forbidden_terms = (
            "impossible",
            "impossibility",
            "guilt",
            "guilty",
            "intent",
            "probability",
            "probable",
            "conclusive",
            "proves",
            "proof",
            "lied",
            "deception",
            "fraud",
        )
        combined_text = " ".join(
            [
                lead.lead_type.value,
                lead.explanation,
                *lead.limitations,
                result.methodology_note,
            ]
        ).lower()

        for term in forbidden_terms:
            assert term not in combined_text, f"Forbidden term '{term}' found in lead output."
        assert lead.relationship.weight is None
        assert lead.relationship.is_observed is False
        assert lead.relationship.relation_type == "CONTRADICTS"


class TestPhase2C1ContradictionLogicAudit:
    """Tests for Phase 2C.1 Audit 1-4: Existential temporal activity-duration feasibility
    and boundary cases.
    """

    def test_activity_duration_fits_exactly_before_b(self) -> None:
        # 1. Exact fit at inclusive lower bound B_start (A_start + D == B_start):
        # A window [10:00:00Z, 10:00:01Z) with duration D = 1800.0s completes at 10:30:00Z == B_start
        # of B window [10:30:00Z, 10:30:01Z). Choosing t_A = 10:00:00Z and t_B = 10:30:00Z satisfies
        # t_A + D == t_B with zero slack -> Feasible (0 leads)!
        win_a = normalize_event_time(
            {
                "event_id": "evt-a",
                "case_id": "case-fit",
                "title": "Origin Activity",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1800.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        win_b = normalize_event_time(
            {
                "event_id": "evt-b",
                "case_id": "case-fit",
                "title": "Subsequent Event",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            }
        )
        feasible_zero, _, post_max_zero, _ = check_temporal_activity_feasibility(
            win_a, win_b, distance_km=0.0, speed_threshold_kmh=180.0
        )
        assert post_max_zero == 1.0
        assert feasible_zero is True

        # 2. Exclusive upper bound B_end reached (A_start + D == B_end -> post_max_elapsed == 0.0s):
        # Since [B_start, B_end) is half-open (t_B < B_end), no t_B in [B_start, B_end) can be >= B_end.
        win_a_exclusive = normalize_event_time(
            {
                "event_id": "evt-a-excl",
                "case_id": "case-fit",
                "title": "Origin Reaching B_end",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1801.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_excl, _, post_max_excl, _ = check_temporal_activity_feasibility(
            win_a_exclusive, win_b, distance_km=0.0, speed_threshold_kmh=180.0
        )
        assert post_max_excl == 0.0
        assert feasible_excl is False

        # 3. Positive-distance exact travel fit at inclusive B_start (A_start + D + t_travel == B_start < B_end):
        # A [10:00:00Z, 10:00:01Z) with D = 1440.0s -> earliest completion 10:24:00Z.
        # Travel 18.0 km at 180.0 km/h takes 360.0s -> arrives at 10:30:00Z == B_start (inside [10:30:00Z, 10:30:01Z)).
        # Supremum post_max_elapsed = 361.0s > 360.0s, infimum speed = 18.0 * 3600 / 361.0 < 180.0 -> Feasible!
        win_a_travel_fit = normalize_event_time(
            {
                "event_id": "evt-a-tr-fit",
                "case_id": "case-fit",
                "title": "Origin",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1440.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_tr, _, post_max_tr, post_speed_tr = check_temporal_activity_feasibility(
            win_a_travel_fit, win_b, distance_km=18.0, speed_threshold_kmh=180.0
        )
        assert post_max_tr == 361.0
        assert (post_speed_tr or 0.0) < 180.0
        assert feasible_tr is True

        # Conversely, if D = 1441.0s (earliest completion 10:24:01Z), then 360.0s travel reaches
        # 10:30:01Z == B_end (exclusive upper bound, unattained), so infimum speed == 180.0 km/h
        # and every t_B in [10:30:00Z, 10:30:01Z) requires speed > 180.0 km/h -> Infeasible!
        win_a_travel_unattained = normalize_event_time(
            {
                "event_id": "evt-a-tr-unatt",
                "case_id": "case-fit",
                "title": "Origin Unattained",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1441.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_unatt, _, post_max_unatt, post_speed_unatt = check_temporal_activity_feasibility(
            win_a_travel_unattained, win_b, distance_km=18.0, speed_threshold_kmh=180.0
        )
        assert post_max_unatt == 360.0
        assert post_speed_unatt == 180.0
        assert feasible_unatt is False

    def test_activity_duration_fits_for_some_timestamp_combinations_but_not_others(self) -> None:
        # Event A: MINUTE precision at 10:00:50Z -> window [10:00:00Z, 10:01:00Z), duration D = 600s (10m).
        # Event B: MINUTE precision at 10:10:10Z -> window [10:10:00Z, 10:11:00Z), ~1.0 km away.
        # - Nominal combination fails: 10:00:50Z + 600s = 10:10:50Z > 10:10:10Z (-40s).
        # - Worst-case combination fails: 10:01:00Z + 600s = 10:11:00Z vs 10:10:00Z (post_min = -60s).
        # - Best-case combination succeeds: 10:00:00Z + 600s = 10:10:00Z vs 10:11:00Z (post_max = +60s,
        #   allowing 1.0 km travel at ~60 km/h <= 180 km/h).
        # Existential rule must NOT generate a false-positive contradiction lead!
        events = [
            {
                "event_id": "evt-a-min",
                "case_id": "case-partial-fit",
                "title": "10-Minute Call at Point A",
                "timestamp": "2026-11-10T10:00:50Z",
                "timestamp_precision": "MINUTE",
                "duration_seconds": 600.0,
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-b-min",
                "case_id": "case-partial-fit",
                "title": "Sighting ~1km Away",
                "timestamp": "2026-11-10T10:10:10Z",
                "timestamp_precision": "MINUTE",
                "coordinates": {"lat": 28.6200, "lon": 77.2150},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]
        result = detect_contradictions(events, speed_threshold_kmh=180.0)
        assert result.leads == []
        assert result.relationships == []

    def test_zero_distance_and_unlocated_activity_duration_exceedance_and_non_positive_travel_time(
        self,
    ) -> None:
        # 1. Zero-distance events where duration fits -> 0 leads
        zero_dist_ok = [
            {
                "event_id": "evt-z1",
                "case_id": "case-z",
                "title": "Stationary A",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 300.0,
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-z2",
                "case_id": "case-z",
                "title": "Stationary B",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]
        assert detect_contradictions(zero_dist_ok).leads == []

        # 2. Zero-distance events where duration exceeds all available time before B -> 1 lead
        zero_dist_exceed = [
            {**zero_dist_ok[0], "duration_seconds": 1200.0},
            zero_dist_ok[1],
        ]
        z_res = detect_contradictions(zero_dist_exceed)
        assert len(z_res.leads) == 1
        assert z_res.leads[0].lead_type == ContradictionLeadType.TEMPORAL_ACTIVITY_OVERLAP_LEAD
        assert z_res.leads[0].derived_inference.distance_km == 0.0
        assert (z_res.leads[0].derived_inference.post_activity_max_elapsed_seconds or 0.0) < 0.0

        # 3. Missing coordinates (unlocated) where duration exceeds all available time before B -> 1 lead
        unlocated_exceed = [
            {**zero_dist_ok[0], "coordinates": None, "duration_seconds": 1200.0},
            {**zero_dist_ok[1], "coordinates": None},
        ]
        u_res = detect_contradictions(unlocated_exceed)
        assert len(u_res.leads) == 1
        assert u_res.leads[0].lead_type == ContradictionLeadType.TEMPORAL_ACTIVITY_OVERLAP_LEAD
        assert u_res.leads[0].derived_inference.distance_km is None
        assert u_res.leads[0].observed_facts.origin_coordinates is None
        assert u_res.leads[0].observed_facts.destination_coordinates is None

        # 4. Non-positive available travel time (post_max_elapsed == 0.0s) with positive distance d > 0 -> 1 lead
        non_pos_travel = [
            {
                "event_id": "evt-np-1",
                "case_id": "case-np",
                "title": "Origin With Exact Duration Reaching B_end",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 601.0,  # reaches 10:10:01Z == B_end
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-np-2",
                "case_id": "case-np",
                "title": "Destination 5km Away at 10:10:00Z",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",  # [10:10:00Z, 10:10:01Z)
                "coordinates": {"lat": 28.6500, "lon": 77.2500},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]
        np_res = detect_contradictions(non_pos_travel)
        assert len(np_res.leads) == 1
        assert np_res.leads[0].derived_inference.post_activity_max_elapsed_seconds == 0.0
        assert np_res.leads[0].derived_inference.post_activity_min_required_speed_kmh is None

    def test_timezone_normalized_utc_intervals_in_contradiction_detection(self) -> None:
        # Event 1: 2026-11-10T15:32:00 naive in Asia/Kolkata (+05:30) with -120s clock offset -> 10:00:00Z
        # Event 2: 2026-11-10T05:05:00 naive in America/New_York (-05:00) -> 10:05:00Z (5 minutes later!)
        events = [
            {
                "event_id": "evt-ist",
                "case_id": "case-tz",
                "title": "Delhi CCTV Sighting (IST)",
                "timestamp": "2026-11-10T15:32:00",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-ist",
            },
            {
                "event_id": "evt-est",
                "case_id": "case-tz",
                "title": "New York Access Log (EST)",
                "timestamp": "2026-11-10T05:05:00",
                "timestamp_precision": "EXACT_SECOND",
                "coordinates": {"lat": 40.7128, "lon": -74.0060},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-est",
            },
        ]
        evidence_by_id = {
            "ev-ist": {
                "metadata": {
                    "source_timezone": "Asia/Kolkata",
                    "clock_offset_seconds": -120.0,
                }
            },
            "ev-est": {
                "metadata": {
                    "source_timezone": "America/New_York",
                    "clock_offset_seconds": 0.0,
                }
            },
        }

        result = detect_contradictions(events, evidence_by_id=evidence_by_id)
        assert len(result.leads) == 1
        lead = result.leads[0]
        assert lead.origin_event_id == "evt-ist"
        assert lead.destination_event_id == "evt-est"
        assert lead.derived_inference.max_possible_elapsed_seconds == 301.0
        assert lead.relationship.relation_type == "CONTRADICTS"
        assert lead.relationship.is_observed is False


class TestPhase2C2HalfOpenFeasibilityBoundaryAudit:
    """Dedicated boundary test suite for Phase 2C.2 half-open [start, end) feasibility."""

    def test_boundary_1_and_2_a_start_plus_d_equals_vs_less_than_b_end(self) -> None:
        # B is [10:30:00Z, 10:31:00Z) (MINUTE precision, B_start=10:30:00Z, B_end=10:31:00Z)
        win_b = normalize_event_time(
            {
                "event_id": "evt-b",
                "case_id": "case-2c2",
                "title": "Event B",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "MINUTE",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            }
        )

        # Boundary 1: A_start (10:00:00Z) + D (1860.0s) == B_end (10:31:00Z) -> no feasible B in [B_start, B_end)
        win_a_eq_end = normalize_event_time(
            {
                "event_id": "evt-a-eq",
                "case_id": "case-2c2",
                "title": "Event A Reaching B_end",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1860.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_eq, _, post_max_eq, _ = check_temporal_activity_feasibility(
            win_a_eq_end, win_b, distance_km=0.0
        )
        assert post_max_eq == 0.0
        assert feasible_eq is False

        # Boundary 2: A_start (10:00:00Z) + D (1859.0s) < B_end (10:31:00Z) -> feasible B in [10:30:59Z, 10:31:00Z)
        win_a_lt_end = normalize_event_time(
            {
                "event_id": "evt-a-lt",
                "case_id": "case-2c2",
                "title": "Event A Just Before B_end",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1859.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_lt, _, post_max_lt, _ = check_temporal_activity_feasibility(
            win_a_lt_end, win_b, distance_km=0.0
        )
        assert post_max_lt == 1.0
        assert feasible_lt is True

    def test_boundary_3_and_4_a_start_plus_d_equals_b_start_vs_inside_b_window(self) -> None:
        # B is [10:30:00Z, 10:31:00Z) (B_start = 10:30:00Z, B_end = 10:31:00Z)
        win_b = normalize_event_time(
            {
                "event_id": "evt-b",
                "case_id": "case-2c2",
                "title": "Event B",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "MINUTE",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            }
        )

        # Boundary 3: A_start (10:00:00Z) + D (1800.0s) == B_start (10:30:00Z)
        win_a_eq_start = normalize_event_time(
            {
                "event_id": "evt-a-start",
                "case_id": "case-2c2",
                "title": "Exact Completion at B_start",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1800.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_3, _, post_max_3, _ = check_temporal_activity_feasibility(
            win_a_eq_start, win_b, distance_km=0.0
        )
        assert post_max_3 == 60.0
        assert feasible_3 is True

        # Boundary 4: B_start (10:30:00Z) < A_start + D (10:30:30Z) < B_end (10:31:00Z)
        # B may still occur later in (10:30:30Z, 10:31:00Z) -> post_max = 30.0s -> Feasible!
        win_a_mid_b = normalize_event_time(
            {
                "event_id": "evt-a-mid",
                "case_id": "case-2c2",
                "title": "Completion Inside B Window",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1830.0,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_4, _, post_max_4, _ = check_temporal_activity_feasibility(
            win_a_mid_b, win_b, distance_km=0.0
        )
        assert post_max_4 == 30.0
        assert feasible_4 is True

        # With 1.0 km travel during that 30.0s remaining sub-window (infimum speed 120 km/h < 180 km/h):
        feasible_4_tr, _, _, speed_4_tr = check_temporal_activity_feasibility(
            win_a_mid_b, win_b, distance_km=1.0, speed_threshold_kmh=180.0
        )
        assert speed_4_tr == 120.0
        assert feasible_4_tr is True

    def test_boundary_5_and_6_positive_distance_zero_vs_small_positive_travel_interval(
        self,
    ) -> None:
        win_b = normalize_event_time(
            {
                "event_id": "evt-b",
                "case_id": "case-2c2",
                "title": "Event B",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "EXACT_SECOND",  # [10:30:00Z, 10:30:01Z)
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            }
        )

        # Boundary 5: Positive distance (0.5 km) with exactly zero available travel time (A_start + D == B_end)
        win_a_zero_avail = normalize_event_time(
            {
                "event_id": "evt-a-z",
                "case_id": "case-2c2",
                "title": "Zero Travel Window",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1801.0,  # 10:00:00Z + 1801s = 10:30:01Z == B_end
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        feasible_5, _, post_max_5, post_speed_5 = check_temporal_activity_feasibility(
            win_a_zero_avail, win_b, distance_km=0.5, speed_threshold_kmh=180.0
        )
        assert post_max_5 == 0.0
        assert post_speed_5 is None
        assert feasible_5 is False

        # Boundary 6: Positive distance with a small positive available travel interval (post_max = 10.0s):
        win_a_small_avail = normalize_event_time(
            {
                "event_id": "evt-a-sm",
                "case_id": "case-2c2",
                "title": "Small Travel Window (10s)",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 1791.0,  # 10:00:00Z + 1791s = 10:29:51Z -> 10.0s before B_end (10:30:01Z)
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        # 6a: Distance 0.4 km over 10.0s -> infimum speed = 144.0 km/h < 180.0 km/h -> Feasible!
        feasible_6a, _, post_max_6a, speed_6a = check_temporal_activity_feasibility(
            win_a_small_avail, win_b, distance_km=0.4, speed_threshold_kmh=180.0
        )
        assert post_max_6a == 10.0
        assert speed_6a == 144.0
        assert feasible_6a is True

        # 6b: Distance 5.0 km over 10.0s -> infimum speed = 1800.0 km/h >= 180.0 km/h -> Infeasible!
        feasible_6b, _, post_max_6b, speed_6b = check_temporal_activity_feasibility(
            win_a_small_avail, win_b, distance_km=5.0, speed_threshold_kmh=180.0
        )
        assert post_max_6b == 10.0
        assert speed_6b == 1800.0
        assert feasible_6b is False

    def test_boundary_7_and_8_zero_duration_activity_and_invalid_empty_intervals(self) -> None:
        # Boundary 7: Zero-duration activity (duration_seconds = 0.0)
        events_zero_dur = [
            {
                "event_id": "evt-zd-1",
                "case_id": "case-zd",
                "title": "Instantaneous Ping A",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-zd-2",
                "case_id": "case-zd",
                "title": "Instantaneous Ping B (Nearby)",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "duration_seconds": 0.0,
                "coordinates": {"lat": 28.6200, "lon": 77.2150},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]
        assert detect_contradictions(events_zero_dur).leads == []

        # Boundary 8: Empty or inverted half-open intervals rejected by NormalizedTimeWindow and feasibility check
        t0 = datetime(2026, 11, 10, 10, 0, 0, tzinfo=timezone.utc)
        t1 = datetime(2026, 11, 10, 9, 59, 0, tzinfo=timezone.utc)

        with pytest.raises(ValueError, match="half-open interval"):
            NormalizedTimeWindow(
                nominal_utc=t0,
                precision=CanonicalTimePrecision.EXACT_SECOND,
                raw_precision_input="EXACT_SECOND",
                window_start_utc=t0,
                window_end_utc=t0,  # empty [t0, t0)
                uncertainty_span_seconds=1.0,
            )

        with pytest.raises(ValueError, match="half-open interval"):
            NormalizedTimeWindow(
                nominal_utc=t0,
                precision=CanonicalTimePrecision.EXACT_SECOND,
                raw_precision_input="EXACT_SECOND",
                window_start_utc=t0,
                window_end_utc=t1,  # inverted [t0, t1) with t1 < t0
                uncertainty_span_seconds=1.0,
            )

        # Even if bypassed via model_construct(), check_temporal_activity_feasibility rejects empty intervals
        bypassed_empty = NormalizedTimeWindow.model_construct(
            nominal_utc=t0,
            precision=CanonicalTimePrecision.EXACT_SECOND,
            raw_precision_input="EXACT_SECOND",
            window_start_utc=t0,
            window_end_utc=t0,
            uncertainty_span_seconds=0.0,
            duration_seconds=10.0,
        )
        valid_win = normalize_event_time(events_zero_dur[1])
        with pytest.raises(ValueError, match="half-open interval"):
            check_temporal_activity_feasibility(bypassed_empty, valid_win)


