"""Unit tests for the Member 4 Spatial Reconstruction Engine.

All tests use fixed, deterministic timestamps and coordinates.
"""

from copy import deepcopy
from datetime import datetime, timezone
import itertools
import math

import pytest

from app.modules.reconstruction.spatial import (
    DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
    EARTH_MEAN_RADIUS_KM,
    build_entity_trajectories,
    calculate_transit_metrics,
    create_movement_segment,
    haversine_distance_km,
    reconstruct_spatial_trajectories,
    validate_geopoint,
)
from app.modules.reconstruction.temporal import normalize_event_time, reconstruct_timeline
from app.schemas.reconstruction import (
    GeoPoint,
    SpeedThresholdEvaluation,
    TemporalOrderingStatus,
)


class TestCoordinateValidationAndHaversine:
    """Tests for WGS84 coordinate validation and Haversine distance computation."""

    def test_valid_wgs84_coordinates_and_boundary_extremes(self) -> None:
        pt = validate_geopoint({"lat": 28.6139, "lon": 77.2090})
        assert pt == GeoPoint(lat=28.6139, lon=77.2090)

        # Boundary extremes [-90, 90] and [-180, 180]
        assert validate_geopoint({"lat": -90.0, "lon": -180.0}) == GeoPoint(
            lat=-90.0, lon=-180.0
        )
        assert validate_geopoint({"lat": 90.0, "lon": 180.0}) == GeoPoint(
            lat=90.0, lon=180.0
        )
        assert validate_geopoint(None) is None

    def test_does_not_silently_swap_latitude_and_longitude(self) -> None:
        # Both within [-90, 90]: values must remain in their exact lat/lon fields
        pt = validate_geopoint({"lat": 77.2090, "lon": 28.6139})
        assert pt is not None
        assert pt.lat == 77.2090
        assert pt.lon == 28.6139

        # Swapped value where lat=120.0 (> 90) and lon=28.6 must raise ValueError, never swap
        with pytest.raises(ValueError, match="latitude"):
            validate_geopoint({"lat": 120.0, "lon": 28.6139})

    @pytest.mark.parametrize(
        "invalid_coords",
        [
            {"lat": 90.0001, "lon": 0.0},
            {"lat": -90.0001, "lon": 0.0},
            {"lat": 0.0, "lon": 180.0001},
            {"lat": 0.0, "lon": -180.0001},
            {"lat": math.nan, "lon": 77.2},
            {"lat": 28.6, "lon": math.nan},
            {"lat": math.inf, "lon": 77.2},
            {"lat": 28.6, "lon": -math.inf},
            {"lat": True, "lon": 77.2},
            {"lat": 28.6, "lon": False},
            {"lat": "28.6", "lon": 77.2},
            {"lat": 28.6},  # missing lon
            {"lon": 77.2},  # missing lat
            {"latitude": 28.6, "longitude": 77.2},  # wrong key names
            {"lat": 28.6, "lon": 77.2, "extra": 1.0},  # extra key
            [28.6139, 77.2090],  # positional list rejected to prevent (lon, lat) mixups
            (28.6139, 77.2090),
            "28.6139,77.2090",
            12345,
        ],
    )
    def test_invalid_coordinates_nan_and_infinity_rejected(
        self, invalid_coords: object
    ) -> None:
        with pytest.raises(ValueError):
            validate_geopoint(invalid_coords)

    def test_known_haversine_distances_and_identical_coordinates(self) -> None:
        delhi = {"lat": 28.6139, "lon": 77.2090}
        mumbai = {"lat": 19.0760, "lon": 72.8777}

        # Identical coordinates -> exact 0.0 km
        assert haversine_distance_km(delhi, delhi) == 0.0

        # Antimeridian equivalence (0, 180) and (0, -180) -> 0.0 km
        assert haversine_distance_km({"lat": 0.0, "lon": 180.0}, {"lat": 0.0, "lon": -180.0}) == 0.0

        # 1 degree along the equator -> pi / 180 * EARTH_MEAN_RADIUS_KM
        equator_1deg = haversine_distance_km({"lat": 0.0, "lon": 0.0}, {"lat": 0.0, "lon": 1.0})
        expected_1deg = (math.pi / 180.0) * EARTH_MEAN_RADIUS_KM
        assert math.isclose(equator_1deg, expected_1deg, rel_tol=1e-12)

        # Antipodal points -> pi * EARTH_MEAN_RADIUS_KM
        antipodal = haversine_distance_km({"lat": 0.0, "lon": 0.0}, {"lat": 0.0, "lon": 180.0})
        assert math.isclose(antipodal, math.pi * EARTH_MEAN_RADIUS_KM, rel_tol=1e-12)

        # New Delhi to Mumbai (~1148.09 km) and symmetry
        d_dm = haversine_distance_km(delhi, mumbai)
        d_md = haversine_distance_km(mumbai, delhi)
        assert math.isclose(d_dm, d_md, rel_tol=1e-12)
        assert 1145.0 <= d_dm <= 1151.0


class TestTransitMetricsAndZeroNegativeElapsedTime:
    """Tests for conservative elapsed time, minimum required speed, and zero/negative elapsed time."""

    def test_conservative_elapsed_time_and_minimum_speed_calculations(self) -> None:
        # Origin at 10:00 MINUTE precision -> [10:00:00Z, 10:01:00Z), nominal 10:00:30Z
        # Destination at 10:10 MINUTE precision -> [10:10:00Z, 10:11:00Z), nominal 10:10:30Z
        win_a = normalize_event_time(
            {
                "event_id": "evt-a",
                "case_id": "case-1",
                "title": "Origin",
                "timestamp": "2026-11-10T10:00:30Z",
                "timestamp_precision": "MINUTE",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        win_b = normalize_event_time(
            {
                "event_id": "evt-b",
                "case_id": "case-1",
                "title": "Destination",
                "timestamp": "2026-11-10T10:10:30Z",
                "timestamp_precision": "MINUTE",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            }
        )

        # Nominal elapsed: 10m = 600s
        # Min possible elapsed: 10:10:00Z - 10:01:00Z = 540s (9m)
        # Conservative max possible elapsed: 10:11:00Z - 10:00:00Z = 660s (11m)
        # For distance = 11.0 km:
        # Conservative min_required_speed_kmh = 11.0 / (660 / 3600) = 60.0 km/h
        # Nominal speed = 11.0 / (600 / 3600) = 66.0 km/h
        metrics = calculate_transit_metrics(win_a, win_b, distance_km=11.0, speed_threshold_kmh=80.0)
        assert metrics.nominal_elapsed_seconds == 600.0
        assert metrics.min_possible_elapsed_seconds == 540.0
        assert metrics.max_possible_elapsed_seconds == 660.0
        assert math.isclose(metrics.min_required_speed_kmh or 0.0, 60.0, rel_tol=1e-9)
        assert math.isclose(metrics.nominal_speed_kmh or 0.0, 66.0, rel_tol=1e-9)
        assert metrics.exceeds_configured_speed_threshold is False
        assert metrics.threshold_evaluation == SpeedThresholdEvaluation.WITHIN_CONFIGURED_THRESHOLD

        # With a lower configurable speed threshold (50.0 km/h), exceeds_configured_speed_threshold is True
        metrics_low_thresh = calculate_transit_metrics(
            win_a, win_b, distance_km=11.0, speed_threshold_kmh=50.0
        )
        assert metrics_low_thresh.exceeds_configured_speed_threshold is True
        assert (
            metrics_low_thresh.threshold_evaluation
            == SpeedThresholdEvaluation.EXCEEDS_CONFIGURED_THRESHOLD
        )
        assert metrics_low_thresh.configured_speed_threshold_kmh == 50.0

    def test_zero_and_negative_elapsed_time_edge_cases_handled_safely(self) -> None:
        win_early = normalize_event_time(
            {
                "event_id": "evt-1",
                "case_id": "case-1",
                "title": "Early",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",  # [10:00:00Z, 10:00:01Z)
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            }
        )
        win_late = normalize_event_time(
            {
                "event_id": "evt-2",
                "case_id": "case-1",
                "title": "Late",
                "timestamp": "2026-11-10T10:05:00Z",
                "timestamp_precision": "EXACT_SECOND",  # [10:05:00Z, 10:05:01Z)
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            }
        )

        # 1. Negative elapsed time (origin is win_late, destination is win_early) with positive distance
        neg_metrics = calculate_transit_metrics(win_late, win_early, distance_km=5.0)
        assert neg_metrics.max_possible_elapsed_seconds < 0.0
        assert neg_metrics.min_required_speed_kmh is None
        assert neg_metrics.nominal_speed_kmh is None
        assert neg_metrics.exceeds_configured_speed_threshold is True
        assert (
            neg_metrics.threshold_evaluation
            == SpeedThresholdEvaluation.NON_POSITIVE_ELAPSED_TIME
        )

        # 2. Negative elapsed time with zero distance -> stationary 0.0 km/h
        neg_zero_dist = calculate_transit_metrics(win_late, win_early, distance_km=0.0)
        assert neg_zero_dist.min_required_speed_kmh == 0.0
        assert neg_zero_dist.exceeds_configured_speed_threshold is False
        assert (
            neg_zero_dist.threshold_evaluation
            == SpeedThresholdEvaluation.ZERO_DISTANCE_STATIONARY
        )

        # 3. Adjacent half-open windows [10:00:00Z, 10:00:01Z) and [10:00:01Z, 10:00:02Z):
        # min_possible_elapsed_seconds is exactly 0.0s, max_possible_elapsed_seconds is 2.0s
        win_adjacent = normalize_event_time(
            {
                "event_id": "evt-adj",
                "case_id": "case-1",
                "title": "Adjacent Second",
                "timestamp": "2026-11-10T10:00:01Z",
                "timestamp_precision": "EXACT_SECOND",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            }
        )
        adj_metrics = calculate_transit_metrics(win_early, win_adjacent, distance_km=0.01)
        assert adj_metrics.min_possible_elapsed_seconds == 0.0
        assert adj_metrics.max_possible_elapsed_seconds == 2.0
        assert math.isclose(adj_metrics.min_required_speed_kmh or 0.0, 18.0, rel_tol=1e-9)


class TestEntityGroupingAndTrajectoryReconstruction:
    """Tests for entity grouping, unlocated event preservation, overlaps, determinism, and immutability."""

    def test_multiple_entities_sharing_events_and_missing_coordinates_preserved(self) -> None:
        events = [
            {
                "event_id": "evt-1",
                "case_id": "case-multi",
                "title": "Shared Meeting at India Gate",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "India Gate",
                "coordinates": {"lat": 28.6129, "lon": 77.2295},
                "involved_entity_ids": ["ent-1", "ent-2"],
                "source_evidence_id": "ev-cctv-1",
            },
            {
                "event_id": "evt-2",
                "case_id": "case-multi",
                "title": "Unlocated Encrypted Chat by ent-1",
                "timestamp": "2026-11-10T10:15:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": None,
                "coordinates": None,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-chat-1",
            },
            {
                "event_id": "evt-3",
                "case_id": "case-multi",
                "title": "ent-1 Sighting at Connaught Place",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Connaught Place",
                "coordinates": {"lat": 28.6315, "lon": 77.2167},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-cctv-2",
            },
            {
                "event_id": "evt-4",
                "case_id": "case-multi",
                "title": "ent-2 CDR Ping at Hauz Khas",
                "timestamp": "2026-11-10T10:45:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Hauz Khas",
                "coordinates": {"lat": 28.5494, "lon": 77.2001},
                "involved_entity_ids": ["ent-2"],
                "source_evidence_id": "ev-cdr-1",
            },
        ]

        trajectories = build_entity_trajectories(events)
        assert set(trajectories.keys()) == {"ent-1", "ent-2"}

        # Verify ent-1: 3 total timeline entries (including unlocated evt-2), 2 located, 1 segment (evt-1 -> evt-3)
        t1 = trajectories["ent-1"]
        assert [e.event_id for e in t1.timeline_entries] == ["evt-1", "evt-2", "evt-3"]
        assert t1.located_event_ids == ["evt-1", "evt-3"]
        assert t1.unlocated_event_ids == ["evt-2"]
        assert t1.timeline_entries[1].coordinates is None
        assert len(t1.segments) == 1

        seg1 = t1.segments[0]
        assert seg1.origin_event_id == "evt-1"
        assert seg1.destination_event_id == "evt-3"
        assert seg1.origin_source_evidence_id == "ev-cctv-1"
        assert seg1.destination_source_evidence_id == "ev-cctv-2"
        assert seg1.origin_location_name == "India Gate"
        assert seg1.destination_location_name == "Connaught Place"
        assert seg1.origin_coordinates == GeoPoint(lat=28.6129, lon=77.2295)
        assert seg1.destination_coordinates == GeoPoint(lat=28.6315, lon=77.2167)
        assert seg1.distance_km > 0.0
        assert seg1.has_unlocated_intervening_events is True
        assert seg1.intervening_unlocated_event_ids == ["evt-2"]
        assert seg1.continuity_note is not None and "evt-2" in seg1.continuity_note

        # Verify ent-2: 2 located events (evt-1, evt-4), 0 unlocated, 1 segment (evt-1 -> evt-4)
        t2 = trajectories["ent-2"]
        assert [e.event_id for e in t2.timeline_entries] == ["evt-1", "evt-4"]
        assert t2.located_event_ids == ["evt-1", "evt-4"]
        assert t2.unlocated_event_ids == []
        assert len(t2.segments) == 1
        assert t2.segments[0].origin_event_id == "evt-1"
        assert t2.segments[0].destination_event_id == "evt-4"
        assert t2.segments[0].has_unlocated_intervening_events is False
        assert t2.segments[0].intervening_unlocated_event_ids == []
        assert t2.segments[0].continuity_note is None

    def test_overlapping_temporal_windows_do_not_claim_definite_movement_segment(self) -> None:
        events = [
            {
                "event_id": "evt-coarse",
                "case_id": "case-ov",
                "title": "Coarse Hour Witness Sighting",
                "timestamp": "2026-11-10T14:10:00Z",
                "timestamp_precision": "HOUR",  # [14:00:00Z, 15:00:00Z)
                "location_name": "Location Alpha",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-suspect"],
                "source_evidence_id": "ev-witness-1",
            },
            {
                "event_id": "evt-exact",
                "case_id": "case-ov",
                "title": "Exact Toll Booth Scan",
                "timestamp": "2026-11-10T14:35:00Z",
                "timestamp_precision": "EXACT_SECOND",  # [14:35:00Z, 14:35:01Z) - overlaps evt-coarse!
                "location_name": "Location Beta",
                "coordinates": {"lat": 28.5355, "lon": 77.3910},
                "involved_entity_ids": ["ent-suspect"],
                "source_evidence_id": "ev-toll-1",
            },
        ]

        result = reconstruct_spatial_trajectories(events)
        traj = result.trajectories_by_entity["ent-suspect"]

        # No directed MovementSegment may be claimed when uncertainty windows overlap
        assert traj.segments == []
        assert traj.total_strictly_ordered_distance_km == 0.0
        assert len(traj.uncertain_spatial_overlaps) == 1

        ov = traj.uncertain_spatial_overlaps[0]
        assert ov.event_a_id == "evt-coarse"
        assert ov.event_b_id == "evt-exact"
        assert ov.source_evidence_a_id == "ev-witness-1"
        assert ov.source_evidence_b_id == "ev-toll-1"
        assert ov.ordering_status == TemporalOrderingStatus.CONCURRENT_UNCERTAIN
        assert ov.overlap_start_utc == datetime(2026, 11, 10, 14, 35, 0, tzinfo=timezone.utc)
        assert ov.overlap_end_utc == datetime(2026, 11, 10, 14, 35, 1, tzinfo=timezone.utc)
        assert ov.distance_km > 15.0

        # Direct call to create_movement_segment on overlapping entries must also raise ValueError
        timeline = reconstruct_timeline(events)
        with pytest.raises(ValueError, match="overlap"):
            create_movement_segment(timeline.entries[0], timeline.entries[1], "ent-suspect")

    def test_deterministic_output_and_input_immutability(self) -> None:
        raw_events = [
            {
                "event_id": "evt-3",
                "case_id": "case-det",
                "title": "Stop 3",
                "timestamp": "2026-11-10T12:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Loc 3",
                "coordinates": {"lat": 28.7041, "lon": 77.1025},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-3",
            },
            {
                "event_id": "evt-1",
                "case_id": "case-det",
                "title": "Stop 1",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Loc 1",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-2",
                "case_id": "case-det",
                "title": "Stop 2",
                "timestamp": "2026-11-10T11:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Loc 2",
                "coordinates": {"lat": 28.5355, "lon": 77.3910},
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

        baseline = reconstruct_spatial_trajectories(raw_events, evidence_by_id=raw_evidence)

        # Verify inputs were never mutated
        assert raw_events == events_snapshot
        assert raw_evidence == evidence_snapshot

        # Verify all permutations yield identical result
        for perm in itertools.permutations(raw_events):
            perm_result = reconstruct_spatial_trajectories(
                list(perm), evidence_by_id=raw_evidence
            )
            assert perm_result == baseline

        traj = baseline.trajectories_by_entity["ent-1"]
        assert [s.origin_event_id for s in traj.segments] == ["evt-1", "evt-2"]
        assert [s.destination_event_id for s in traj.segments] == ["evt-2", "evt-3"]
        assert math.isclose(
            traj.total_strictly_ordered_distance_km,
            sum(s.distance_km for s in traj.segments),
            rel_tol=1e-12,
        )


class TestPhase2B1SpatialSemanticsAudit:
    """Regression tests for Phase 2B.1 Audit A (overlap isolation), Audit B (missing-location gaps),
    and Audit C (threshold semantics).
    """

    def test_audit_a_overlap_isolation_four_events(self) -> None:
        # Four events for one entity:
        # A and B have overlapping uncertainty windows [10:00Z, 11:00Z) and [10:25:00Z, 10:25:01Z).
        # C and D have strictly ordered, non-overlapping windows [12:00:00Z, 12:00:01Z) and [12:30:00Z, 12:30:01Z).
        events = [
            {
                "event_id": "evt-A",
                "case_id": "case-audit-a",
                "title": "Sighting A (Coarse Hour)",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "HOUR",
                "location_name": "Loc A",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-target"],
                "source_evidence_id": "ev-A",
            },
            {
                "event_id": "evt-B",
                "case_id": "case-audit-a",
                "title": "Sighting B (Overlaps A)",
                "timestamp": "2026-11-10T10:25:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Loc B",
                "coordinates": {"lat": 28.6315, "lon": 77.2167},
                "involved_entity_ids": ["ent-target"],
                "source_evidence_id": "ev-B",
            },
            {
                "event_id": "evt-C",
                "case_id": "case-audit-a",
                "title": "Sighting C (Strictly Ordered)",
                "timestamp": "2026-11-10T12:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Loc C",
                "coordinates": {"lat": 28.5355, "lon": 77.3910},
                "involved_entity_ids": ["ent-target"],
                "source_evidence_id": "ev-C",
            },
            {
                "event_id": "evt-D",
                "case_id": "case-audit-a",
                "title": "Sighting D (Strictly Ordered After C)",
                "timestamp": "2026-11-10T12:30:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Loc D",
                "coordinates": {"lat": 28.4595, "lon": 77.0266},
                "involved_entity_ids": ["ent-target"],
                "source_evidence_id": "ev-D",
            },
        ]

        result = reconstruct_spatial_trajectories(events)
        traj = result.trajectories_by_entity["ent-target"]

        # 1. A and B are marked uncertain in uncertain_spatial_overlaps
        assert len(traj.uncertain_spatial_overlaps) == 1
        ov = traj.uncertain_spatial_overlaps[0]
        assert {ov.event_a_id, ov.event_b_id} == {"evt-A", "evt-B"}
        assert ov.ordering_status == TemporalOrderingStatus.CONCURRENT_UNCERTAIN

        # 2. No arbitrary movement segment is inferred between A and B (or from A/B to C)
        # 3. C -> D still produces a valid movement segment because its ordering is independently defensible
        assert len(traj.segments) == 1
        seg = traj.segments[0]
        assert seg.origin_event_id == "evt-C"
        assert seg.destination_event_id == "evt-D"
        assert seg.ordering_status == TemporalOrderingStatus.STRICTLY_ORDERED

    def test_audit_b_missing_location_gaps_explicitly_recorded(self) -> None:
        # A: located event (10:00Z)
        # B: unlocated event (10:15Z, coordinates=None)
        # C: located event (10:30Z, chronologically after B)
        events = [
            {
                "event_id": "evt-A",
                "case_id": "case-audit-b",
                "title": "Origin Camera A",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Point A",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-B",
                "case_id": "case-audit-b",
                "title": "Unlocated Phone Call B",
                "timestamp": "2026-11-10T10:15:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": None,
                "coordinates": None,
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
            {
                "event_id": "evt-C",
                "case_id": "case-audit-b",
                "title": "Destination Camera C",
                "timestamp": "2026-11-10T10:30:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Point C",
                "coordinates": {"lat": 28.7041, "lon": 77.1025},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-3",
            },
        ]

        result = reconstruct_spatial_trajectories(events)
        traj = result.trajectories_by_entity["ent-1"]

        assert len(traj.segments) == 1
        seg = traj.segments[0]
        assert seg.origin_event_id == "evt-A"
        assert seg.destination_event_id == "evt-C"
        assert seg.has_unlocated_intervening_events is True
        assert seg.intervening_unlocated_event_ids == ["evt-B"]
        assert seg.continuity_note is not None
        assert "evt-B" in seg.continuity_note
        assert "not an uninterrupted path" in seg.continuity_note

    def test_audit_c_threshold_semantics_never_suppresses_or_labels_impossible(self) -> None:
        assert DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH == 180.0

        # Two strictly ordered events ~1148 km apart (Delhi -> Mumbai) separated by 600 seconds (10 min)
        events = [
            {
                "event_id": "evt-delhi",
                "case_id": "case-audit-c",
                "title": "Delhi Sighting",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "New Delhi",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-mumbai",
                "case_id": "case-audit-c",
                "title": "Mumbai Sighting",
                "timestamp": "2026-11-10T10:10:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "location_name": "Mumbai",
                "coordinates": {"lat": 19.0760, "lon": 72.8777},
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-2",
            },
        ]

        result = reconstruct_spatial_trajectories(events)
        traj = result.trajectories_by_entity["ent-1"]

        # Segment is NOT suppressed by the speed threshold
        assert len(traj.segments) == 1
        seg = traj.segments[0]
        assert seg.exceeds_configured_speed_threshold is True
        assert seg.threshold_evaluation == SpeedThresholdEvaluation.EXCEEDS_CONFIGURED_THRESHOLD
        assert "impossible" not in seg.threshold_evaluation.value.lower()
        assert "reference assumption" in seg.threshold_assumption_note

