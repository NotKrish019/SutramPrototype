"""Unit tests for the Member 4 Temporal Reconstruction Engine.

All tests use fixed, deterministic timestamps (never datetime.now()).
"""

from copy import deepcopy
from datetime import date, datetime, timezone
import itertools
import math

import pytest
from pydantic import BaseModel

from app.modules.reconstruction.temporal import (
    DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
    compute_uncertainty_window,
    intervals_overlap,
    normalize_event_time,
    normalize_time_precision,
    parse_and_normalize_timestamp,
    reconstruct_timeline,
)
from app.schemas.reconstruction import (
    CanonicalTimePrecision,
    TemporalOrderingStatus,
)


class _MockEventModel(BaseModel):
    """Pydantic model mirroring the committed Event schema for object-input testing."""

    event_id: str
    case_id: str
    title: str
    timestamp: str
    timestamp_precision: str
    location_name: str | None = None
    coordinates: dict[str, float] | None = None
    involved_entity_ids: list[str]
    source_evidence_id: str
    duration_seconds: float | None = None


class TestTimestampNormalization:
    """Tests for ISO 8601 parsing, timezone localization, and clock offset correction."""

    def test_utc_timestamp_ending_in_z(self) -> None:
        result = parse_and_normalize_timestamp("2026-11-10T21:45:12Z")
        assert result == datetime(2026, 11, 10, 21, 45, 12, tzinfo=timezone.utc)
        assert result.tzinfo == timezone.utc

    def test_positive_and_negative_iso_offsets(self) -> None:
        # +05:30 (IST) -> 16:15:12 UTC
        pos = parse_and_normalize_timestamp("2026-11-10T21:45:12+05:30")
        assert pos == datetime(2026, 11, 10, 16, 15, 12, tzinfo=timezone.utc)

        # -04:00 (EDT) -> 18:30:00 UTC
        neg = parse_and_normalize_timestamp("2026-06-15T14:30:00-04:00")
        assert neg == datetime(2026, 6, 15, 18, 30, 0, tzinfo=timezone.utc)

    def test_naive_timestamp_with_asia_kolkata(self) -> None:
        result = parse_and_normalize_timestamp(
            "2026-11-10T21:45:12",
            source_timezone="Asia/Kolkata",
        )
        assert result == datetime(2026, 11, 10, 16, 15, 12, tzinfo=timezone.utc)

    def test_clock_offset_correction_positive_and_negative(self) -> None:
        # DVR clock is 120 seconds fast -> apply -120.0s offset
        corrected_neg = parse_and_normalize_timestamp(
            "2026-11-10T21:47:00",
            source_timezone="Asia/Kolkata",
            clock_offset_seconds=-120.0,
        )
        assert corrected_neg == datetime(2026, 11, 10, 16, 15, 0, tzinfo=timezone.utc)

        # Device clock is 45.5 seconds slow -> apply +45.5s offset
        corrected_pos = parse_and_normalize_timestamp(
            "2026-11-10T10:00:00Z",
            clock_offset_seconds=45.5,
        )
        assert corrected_pos == datetime(2026, 11, 10, 10, 0, 45, 500000, tzinfo=timezone.utc)

    @pytest.mark.parametrize(
        "invalid_tz",
        [
            "Invalid/Timezone",
            "Asia/NotACity",
            "",
            "   ",
            None,
            123,
        ],
    )
    def test_invalid_iana_timezone_rejected(self, invalid_tz: object) -> None:
        with pytest.raises(ValueError, match="timezone"):
            parse_and_normalize_timestamp(
                "2026-11-10T21:45:12",
                source_timezone=invalid_tz,  # type: ignore[arg-type]
            )

    @pytest.mark.parametrize(
        "invalid_ts",
        [
            "",
            "   ",
            "not-a-timestamp",
            "2026-13-01T10:00:00Z",
            "2026-11-10",  # date-only without time component
            "20261110",
            None,
            1700000000,
            True,
            date(2026, 11, 10),
        ],
    )
    def test_invalid_and_empty_timestamps_rejected(self, invalid_ts: object) -> None:
        with pytest.raises(ValueError, match="(timestamp|ISO 8601)"):
            parse_and_normalize_timestamp(invalid_ts)  # type: ignore[arg-type]

    @pytest.mark.parametrize(
        "invalid_offset",
        [
            math.nan,
            math.inf,
            -math.inf,
            True,
            False,
            "10",
            None,
        ],
    )
    def test_invalid_clock_offsets_rejected(self, invalid_offset: object) -> None:
        with pytest.raises(ValueError, match="clock_offset_seconds"):
            parse_and_normalize_timestamp(
                "2026-11-10T21:45:12Z",
                clock_offset_seconds=invalid_offset,  # type: ignore[arg-type]
            )


class TestPrecisionAndUncertaintyWindows:
    """Tests for precision-based and explicit half-open uncertainty windows [start, end)."""

    def test_every_supported_canonical_precision_and_aliases(self) -> None:
        # 1. EXACT_SECOND -> [21:45:12Z, 21:45:13Z) (1s)
        nom, start, end = compute_uncertainty_window(
            "2026-11-10T21:45:12.789000Z",
            CanonicalTimePrecision.EXACT_SECOND,
        )
        assert nom == datetime(2026, 11, 10, 21, 45, 12, 789000, tzinfo=timezone.utc)
        assert start == datetime(2026, 11, 10, 21, 45, 12, tzinfo=timezone.utc)
        assert end == datetime(2026, 11, 10, 21, 45, 13, tzinfo=timezone.utc)

        # 2. MINUTE in Asia/Kolkata -> local [21:45:00, 21:46:00) -> UTC [16:15:00Z, 16:16:00Z)
        nom, start, end = compute_uncertainty_window(
            "2026-11-10T21:45:42",
            CanonicalTimePrecision.MINUTE,
            source_timezone="Asia/Kolkata",
        )
        assert nom == datetime(2026, 11, 10, 16, 15, 42, tzinfo=timezone.utc)
        assert start == datetime(2026, 11, 10, 16, 15, 0, tzinfo=timezone.utc)
        assert end == datetime(2026, 11, 10, 16, 16, 0, tzinfo=timezone.utc)

        # 3. HOUR in Asia/Kolkata -> local [21:00:00, 22:00:00) -> UTC [15:30:00Z, 16:30:00Z)
        nom, start, end = compute_uncertainty_window(
            "2026-11-10T21:45:42",
            CanonicalTimePrecision.HOUR,
            source_timezone="Asia/Kolkata",
        )
        assert start == datetime(2026, 11, 10, 15, 30, 0, tzinfo=timezone.utc)
        assert end == datetime(2026, 11, 10, 16, 30, 0, tzinfo=timezone.utc)

        # 4. DAY in Asia/Kolkata -> local [2026-11-10 00:00, 2026-11-11 00:00)
        # -> UTC [2026-11-09T18:30:00Z, 2026-11-10T18:30:00Z)
        nom, start, end = compute_uncertainty_window(
            "2026-11-10T21:45:42",
            CanonicalTimePrecision.DAY,
            source_timezone="Asia/Kolkata",
        )
        assert start == datetime(2026, 11, 9, 18, 30, 0, tzinfo=timezone.utc)
        assert end == datetime(2026, 11, 10, 18, 30, 0, tzinfo=timezone.utc)
        assert (end - start).total_seconds() == 86400.0

        # 5. ESTIMATED (default 900s half-width) -> [nominal - 900s, nominal + 900s)
        nom, start, end = compute_uncertainty_window(
            "2026-11-10T12:00:00Z",
            CanonicalTimePrecision.ESTIMATED,
        )
        assert start == datetime(2026, 11, 10, 11, 45, 0, tzinfo=timezone.utc)
        assert end == datetime(2026, 11, 10, 12, 15, 0, tzinfo=timezone.utc)
        assert (end - start).total_seconds() == 2 * DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS

        # 6. Legacy compatibility adapter aliases ('EXACT', 'APPROXIMATE')
        assert normalize_time_precision("EXACT") == CanonicalTimePrecision.EXACT_SECOND
        assert normalize_time_precision("APPROXIMATE") == CanonicalTimePrecision.ESTIMATED
        with pytest.raises(ValueError):
            normalize_time_precision("EXACT", allow_legacy_aliases=False)
        with pytest.raises(ValueError):
            normalize_time_precision("UNKNOWN_PRECISION")

    def test_clock_offset_applied_consistently_to_nominal_and_boundaries(self) -> None:
        nom, start, end = compute_uncertainty_window(
            "2026-11-10T21:47:15",
            "MINUTE",
            source_timezone="Asia/Kolkata",
            clock_offset_seconds=-120.0,
        )
        # Uncorrected local minute: [21:47:00+05:30, 21:48:00+05:30) = [16:17:00Z, 16:18:00Z)
        # With -120s offset: nominal=16:15:15Z, start=16:15:00Z, end=16:16:00Z
        assert nom == datetime(2026, 11, 10, 16, 15, 15, tzinfo=timezone.utc)
        assert start == datetime(2026, 11, 10, 16, 15, 0, tzinfo=timezone.utc)
        assert end == datetime(2026, 11, 10, 16, 16, 0, tzinfo=timezone.utc)

    def test_estimated_uncertainty_half_width_configuration(self) -> None:
        event = {
            "event_id": "evt-est-1",
            "case_id": "case-1",
            "title": "Witness Sighting",
            "timestamp": "2026-11-10T15:00:00Z",
            "timestamp_precision": "ESTIMATED",
            "involved_entity_ids": ["ent-1"],
            "source_evidence_id": "ev-1",
        }
        window = normalize_event_time(event, estimated_half_width_seconds=300.0)
        assert window.is_estimated_window_assumption is True
        assert window.estimated_half_width_seconds == 300.0
        assert window.window_start_utc == datetime(2026, 11, 10, 14, 55, 0, tzinfo=timezone.utc)
        assert window.window_end_utc == datetime(2026, 11, 10, 15, 5, 0, tzinfo=timezone.utc)
        assert window.uncertainty_span_seconds == 600.0

        for bad_width in (0.0, -60.0, math.nan, math.inf, True):
            with pytest.raises(ValueError, match="estimated_half_width_seconds"):
                normalize_event_time(event, estimated_half_width_seconds=bad_width)  # type: ignore[arg-type]

    def test_explicit_uncertainty_bounds_validated_and_normalized(self) -> None:
        event = {
            "event_id": "evt-exp-1",
            "case_id": "case-1",
            "title": "Bounded Event",
            "timestamp": "2026-11-10T21:45:00",
            "timestamp_precision": "ESTIMATED",
            "uncertainty_window_start": "2026-11-10T21:40:00",
            "uncertainty_window_end": "2026-11-10T21:50:00",
            "involved_entity_ids": ["ent-1"],
            "source_evidence_id": "ev-1",
        }
        evidence = {"metadata": {"source_timezone": "Asia/Kolkata", "clock_offset_seconds": -60.0}}
        window = normalize_event_time(event, evidence_metadata=evidence)
        assert window.has_explicit_bounds is True
        assert window.is_estimated_window_assumption is False
        assert window.window_start_utc == datetime(2026, 11, 10, 16, 9, 0, tzinfo=timezone.utc)
        assert window.nominal_utc == datetime(2026, 11, 10, 16, 14, 0, tzinfo=timezone.utc)
        assert window.window_end_utc == datetime(2026, 11, 10, 16, 19, 0, tzinfo=timezone.utc)
        assert window.uncertainty_span_seconds == 600.0

        # Missing one bound fails
        with pytest.raises(ValueError, match="Both explicit_start and explicit_end"):
            compute_uncertainty_window(
                "2026-11-10T12:00:00Z",
                "ESTIMATED",
                explicit_start="2026-11-10T11:50:00Z",
            )

        # Inverted bounds fail
        with pytest.raises(ValueError, match="strictly earlier"):
            compute_uncertainty_window(
                "2026-11-10T12:00:00Z",
                "ESTIMATED",
                explicit_start="2026-11-10T12:10:00Z",
                explicit_end="2026-11-10T11:50:00Z",
            )

        # Nominal outside explicit bounds fails
        with pytest.raises(ValueError, match="outside explicit uncertainty window"):
            compute_uncertainty_window(
                "2026-11-10T12:30:00Z",
                "ESTIMATED",
                explicit_start="2026-11-10T11:50:00Z",
                explicit_end="2026-11-10T12:10:00Z",
            )

    def test_duration_independent_of_uncertainty_and_never_invented(self) -> None:
        # 1. Event without duration_seconds -> duration_seconds is None
        event_no_duration = {
            "event_id": "evt-dur-1",
            "case_id": "case-1",
            "title": "CDR Call",
            "timestamp": "2026-11-10T21:45:12Z",
            "timestamp_precision": "EXACT_SECOND",
            "involved_entity_ids": ["ent-1"],
            "source_evidence_id": "ev-1",
        }
        win_none = normalize_event_time(event_no_duration)
        assert win_none.duration_seconds is None
        assert win_none.uncertainty_span_seconds == 1.0

        # 2. Event with duration_seconds=184.0 -> duration preserved, uncertainty window remains 1.0s
        event_with_duration = {**event_no_duration, "duration_seconds": 184.0}
        win_with_dur = normalize_event_time(event_with_duration)
        assert win_with_dur.duration_seconds == 184.0
        assert win_with_dur.uncertainty_span_seconds == 1.0
        assert win_with_dur.window_start_utc == win_none.window_start_utc
        assert win_with_dur.window_end_utc == win_none.window_end_utc

        # 3. Invalid duration rejected
        with pytest.raises(ValueError, match="duration_seconds"):
            normalize_event_time({**event_no_duration, "duration_seconds": -5.0})

    def test_dst_spring_forward_and_fall_back_local_days(self) -> None:
        # Spring-forward in America/New_York: 2026-03-08 is a 23-hour local calendar day (82,800s)
        _, sf_start, sf_end = compute_uncertainty_window(
            "2026-03-08T12:00:00",
            CanonicalTimePrecision.DAY,
            source_timezone="America/New_York",
        )
        assert sf_start == datetime(2026, 3, 8, 5, 0, 0, tzinfo=timezone.utc)
        assert sf_end == datetime(2026, 3, 9, 4, 0, 0, tzinfo=timezone.utc)
        assert (sf_end - sf_start).total_seconds() == 23 * 3600.0

        # Fall-back in America/New_York: 2026-11-01 is a 25-hour local calendar day (90,000s)
        _, fb_start, fb_end = compute_uncertainty_window(
            "2026-11-01T12:00:00",
            CanonicalTimePrecision.DAY,
            source_timezone="America/New_York",
        )
        assert fb_start == datetime(2026, 11, 1, 4, 0, 0, tzinfo=timezone.utc)
        assert fb_end == datetime(2026, 11, 2, 5, 0, 0, tzinfo=timezone.utc)
        assert (fb_end - fb_start).total_seconds() == 25 * 3600.0

        # Aware UTC timestamp with explicit Evidence source_timezone="America/New_York" and clock offset
        dst_event = {
            "event_id": "evt-dst-aware",
            "case_id": "case-dst",
            "title": "Spring-Forward Day Event",
            "timestamp": "2026-03-08T16:00:00Z",
            "timestamp_precision": "DAY",
            "involved_entity_ids": ["ent-1"],
            "source_evidence_id": "ev-ny",
        }
        dst_evidence = {
            "metadata": {
                "source_timezone": "America/New_York",
                "clock_offset_seconds": 30.0,
            }
        }
        dst_win = normalize_event_time(dst_event, evidence_metadata=dst_evidence)
        assert dst_win.window_start_utc == datetime(2026, 3, 8, 5, 0, 30, tzinfo=timezone.utc)
        assert dst_win.window_end_utc == datetime(2026, 3, 9, 4, 0, 30, tzinfo=timezone.utc)
        assert dst_win.nominal_utc == datetime(2026, 3, 8, 16, 0, 30, tzinfo=timezone.utc)
        assert dst_win.uncertainty_span_seconds == 23 * 3600.0

    @pytest.mark.parametrize(
        "ambiguous_input",
        [
            "2026-11-01 01:30",
            "2026-11-01T01:30:00",
            datetime(2026, 11, 1, 1, 30, 0),
        ],
    )
    def test_dst_fall_back_ambiguous_naive_timestamp_rejected(
        self, ambiguous_input: str | datetime
    ) -> None:
        with pytest.raises(ValueError) as exc_info:
            parse_and_normalize_timestamp(
                ambiguous_input,
                source_timezone="America/New_York",
            )
        msg = str(exc_info.value)
        assert "Ambiguous" in msg
        assert "2026-11-01" in msg and "01:30" in msg
        assert "America/New_York" in msg
        assert "-04:00" in msg and "-05:00" in msg

        # Also rejected in compute_uncertainty_window and reconstruct_timeline
        with pytest.raises(ValueError, match="Ambiguous.*America/New_York"):
            compute_uncertainty_window(
                ambiguous_input,
                CanonicalTimePrecision.MINUTE,
                source_timezone="America/New_York",
            )

        with pytest.raises(ValueError, match="evt-ambig-ny.*Ambiguous.*America/New_York"):
            reconstruct_timeline(
                [
                    {
                        "event_id": "evt-ambig-ny",
                        "case_id": "case-dst",
                        "title": "Ambiguous CCTV Log",
                        "timestamp": "2026-11-01 01:30",
                        "timestamp_precision": "MINUTE",
                        "involved_entity_ids": ["ent-1"],
                        "source_evidence_id": "ev-ny",
                    }
                ],
                evidence_by_id={"ev-ny": {"metadata": {"source_timezone": "America/New_York"}}},
            )

    @pytest.mark.parametrize(
        "nonexistent_input",
        [
            "2026-03-08 02:30",
            "2026-03-08T02:30:00",
            datetime(2026, 3, 8, 2, 30, 0),
        ],
    )
    def test_dst_spring_forward_nonexistent_naive_timestamp_rejected(
        self, nonexistent_input: str | datetime
    ) -> None:
        with pytest.raises(ValueError) as exc_info:
            parse_and_normalize_timestamp(
                nonexistent_input,
                source_timezone="America/New_York",
            )
        msg = str(exc_info.value)
        assert "Nonexistent" in msg
        assert "2026-03-08" in msg and "02:30" in msg
        assert "America/New_York" in msg

        # Also rejected in compute_uncertainty_window and reconstruct_timeline
        with pytest.raises(ValueError, match="Nonexistent.*America/New_York"):
            compute_uncertainty_window(
                nonexistent_input,
                CanonicalTimePrecision.MINUTE,
                source_timezone="America/New_York",
            )

        with pytest.raises(ValueError, match="evt-gap-ny.*Nonexistent.*America/New_York"):
            reconstruct_timeline(
                [
                    {
                        "event_id": "evt-gap-ny",
                        "case_id": "case-dst",
                        "title": "Gap CCTV Log",
                        "timestamp": "2026-03-08 02:30",
                        "timestamp_precision": "MINUTE",
                        "involved_entity_ids": ["ent-1"],
                        "source_evidence_id": "ev-ny",
                    }
                ],
                evidence_by_id={"ev-ny": {"metadata": {"source_timezone": "America/New_York"}}},
            )

    def test_dst_fall_back_disambiguated_by_explicit_iso_offsets(self) -> None:
        # First occurrence (EDT, UTC-4): 2026-11-01T01:30:00-04:00 -> 05:30:00 UTC
        edt_utc = parse_and_normalize_timestamp(
            "2026-11-01T01:30:00-04:00",
            source_timezone="America/New_York",
        )
        assert edt_utc == datetime(2026, 11, 1, 5, 30, 0, tzinfo=timezone.utc)

        # Second occurrence (EST, UTC-5): 2026-11-01T01:30:00-05:00 -> 06:30:00 UTC
        est_utc = parse_and_normalize_timestamp(
            "2026-11-01T01:30:00-05:00",
            source_timezone="America/New_York",
        )
        assert est_utc == datetime(2026, 11, 1, 6, 30, 0, tzinfo=timezone.utc)

        # Verify [start, end) HOUR intervals for both disambiguated occurrences
        _, edt_h_start, edt_h_end = compute_uncertainty_window(
            "2026-11-01T01:30:00-04:00",
            CanonicalTimePrecision.HOUR,
            source_timezone="America/New_York",
            explicit_source_timezone=True,
        )
        assert edt_h_start == datetime(2026, 11, 1, 5, 0, 0, tzinfo=timezone.utc)
        assert edt_h_end == datetime(2026, 11, 1, 6, 0, 0, tzinfo=timezone.utc)

        _, est_h_start, est_h_end = compute_uncertainty_window(
            "2026-11-01T01:30:00-05:00",
            CanonicalTimePrecision.HOUR,
            source_timezone="America/New_York",
            explicit_source_timezone=True,
        )
        assert est_h_start == datetime(2026, 11, 1, 6, 0, 0, tzinfo=timezone.utc)
        assert est_h_end == datetime(2026, 11, 1, 7, 0, 0, tzinfo=timezone.utc)

        # Both disambiguated events are strictly ordered 1 hour apart (no overlap)
        timeline = reconstruct_timeline(
            [
                {
                    "event_id": "evt-fall-est",
                    "case_id": "case-dst",
                    "title": "Second 01:30 Occurrence (EST)",
                    "timestamp": "2026-11-01T01:30:00-05:00",
                    "timestamp_precision": "HOUR",
                    "involved_entity_ids": ["ent-1"],
                    "source_evidence_id": "ev-ny",
                },
                {
                    "event_id": "evt-fall-edt",
                    "case_id": "case-dst",
                    "title": "First 01:30 Occurrence (EDT)",
                    "timestamp": "2026-11-01T01:30:00-04:00",
                    "timestamp_precision": "HOUR",
                    "involved_entity_ids": ["ent-1"],
                    "source_evidence_id": "ev-ny",
                },
            ],
            evidence_by_id={"ev-ny": {"metadata": {"source_timezone": "America/New_York"}}},
        )
        assert [e.event_id for e in timeline.entries] == ["evt-fall-edt", "evt-fall-est"]
        assert timeline.overlaps == []
        assert all(
            e.ordering_status == TemporalOrderingStatus.STRICTLY_ORDERED for e in timeline.entries
        )


class TestTimelineReconstructionAndOverlapSemantics:
    """Tests for half-open boundary semantics, overlap detection, determinism, and immutability."""

    def test_exact_half_open_interval_boundaries(self) -> None:
        # [10:00:00Z, 10:01:00Z) and [10:01:00Z, 10:02:00Z) touch at 10:01:00Z but do NOT overlap
        t0 = datetime(2026, 11, 10, 10, 0, 0, tzinfo=timezone.utc)
        t1 = datetime(2026, 11, 10, 10, 1, 0, tzinfo=timezone.utc)
        t2 = datetime(2026, 11, 10, 10, 2, 0, tzinfo=timezone.utc)
        assert intervals_overlap(t0, t1, t1, t2) is None

        events = [
            {
                "event_id": "evt-minute-1",
                "case_id": "case-boundary",
                "title": "Minute 10:00",
                "timestamp": "2026-11-10T10:00:45Z",
                "timestamp_precision": "MINUTE",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-minute-2",
                "case_id": "case-boundary",
                "title": "Minute 10:01",
                "timestamp": "2026-11-10T10:01:00Z",
                "timestamp_precision": "MINUTE",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
        ]
        result = reconstruct_timeline(events)
        assert len(result.overlaps) == 0
        assert all(
            entry.ordering_status == TemporalOrderingStatus.STRICTLY_ORDERED
            for entry in result.entries
        )
        assert all(entry.concurrent_event_ids == [] for entry in result.entries)

    def test_overlapping_and_non_overlapping_intervals(self) -> None:
        events = [
            {
                "event_id": "evt-hour",
                "case_id": "case-ov",
                "title": "Coarse Hour Log",
                "timestamp": "2026-11-10T10:05:00Z",
                "timestamp_precision": "HOUR",  # [10:00Z, 11:00Z)
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-sec",
                "case_id": "case-ov",
                "title": "Exact Call Within Hour",
                "timestamp": "2026-11-10T10:42:15Z",
                "timestamp_precision": "EXACT_SECOND",  # [10:42:15Z, 10:42:16Z)
                "involved_entity_ids": ["ent-1", "ent-2"],
                "location_name": "South Extension Tower 4B",
                "source_evidence_id": "ev-2",
            },
            {
                "event_id": "evt-later",
                "case_id": "case-ov",
                "title": "Later Event Outside Hour",
                "timestamp": "2026-11-10T11:00:00Z",
                "timestamp_precision": "EXACT_SECOND",  # [11:00:00Z, 11:00:01Z) - adjacent, non-overlapping!
                "involved_entity_ids": ["ent-2"],
                "source_evidence_id": "ev-2",
            },
        ]
        result = reconstruct_timeline(events)
        assert len(result.entries) == 3
        assert len(result.overlaps) == 1

        ov = result.overlaps[0]
        assert ov.event_a_id == "evt-hour"
        assert ov.event_b_id == "evt-sec"
        assert ov.overlap_start_utc == datetime(2026, 11, 10, 10, 42, 15, tzinfo=timezone.utc)
        assert ov.overlap_end_utc == datetime(2026, 11, 10, 10, 42, 16, tzinfo=timezone.utc)
        assert ov.overlap_seconds == 1.0

        by_id = {e.event_id: e for e in result.entries}
        assert by_id["evt-hour"].ordering_status == TemporalOrderingStatus.CONCURRENT_UNCERTAIN
        assert by_id["evt-hour"].concurrent_event_ids == ["evt-sec"]
        assert by_id["evt-sec"].ordering_status == TemporalOrderingStatus.CONCURRENT_UNCERTAIN
        assert by_id["evt-sec"].concurrent_event_ids == ["evt-hour"]
        assert by_id["evt-later"].ordering_status == TemporalOrderingStatus.STRICTLY_ORDERED
        assert by_id["evt-later"].concurrent_event_ids == []

    def test_deterministic_output_regardless_of_input_order(self) -> None:
        base_events = [
            {
                "event_id": "evt-c",
                "case_id": "case-det",
                "title": "Same Second C",
                "timestamp": "2026-11-10T16:15:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-a",
                "case_id": "case-det",
                "title": "Same Second A",
                "timestamp": "2026-11-10T16:15:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "involved_entity_ids": ["ent-1"],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-b",
                "case_id": "case-det",
                "title": "Minute Window Starting 16:15",
                "timestamp": "2026-11-10T16:15:30Z",
                "timestamp_precision": "MINUTE",
                "involved_entity_ids": ["ent-2"],
                "source_evidence_id": "ev-2",
            },
            {
                "event_id": "evt-d",
                "case_id": "case-det",
                "title": "Earlier Event",
                "timestamp": "2026-11-10T12:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "involved_entity_ids": ["ent-3"],
                "source_evidence_id": "ev-1",
            },
        ]

        baseline = reconstruct_timeline(base_events)
        for perm in itertools.permutations(base_events):
            perm_result = reconstruct_timeline(list(perm))
            assert perm_result == baseline

        assert [e.event_id for e in baseline.entries] == ["evt-d", "evt-a", "evt-c", "evt-b"]

    def test_input_immutability_and_pydantic_model_support(self) -> None:
        raw_events = [
            {
                "event_id": "evt-imm-1",
                "case_id": "case-imm",
                "title": "CCTV Frame Sighting",
                "timestamp": "2026-11-10T21:47:00",
                "timestamp_precision": "MINUTE",
                "location_name": "Gate 1",
                "coordinates": {"lat": 28.6139, "lon": 77.2090},
                "involved_entity_ids": ["ent-101"],
                "source_evidence_id": "ev-cctv-1",
            }
        ]
        raw_evidence = {
            "ev-cctv-1": {
                "evidence_id": "ev-cctv-1",
                "case_id": "case-imm",
                "source_type": "CCTV",
                "metadata": {
                    "source_timezone": "Asia/Kolkata",
                    "clock_offset_seconds": -120.0,
                },
            }
        }
        events_snapshot = deepcopy(raw_events)
        evidence_snapshot = deepcopy(raw_evidence)

        result_from_dict = reconstruct_timeline(raw_events, evidence_by_id=raw_evidence)
        assert raw_events == events_snapshot
        assert raw_evidence == evidence_snapshot

        # Also verify Pydantic model instances produce identical output
        model_events = [_MockEventModel(**raw_events[0])]
        result_from_model = reconstruct_timeline(model_events, evidence_by_id=raw_evidence)
        assert result_from_model == result_from_dict

    def test_offending_event_id_reported_on_failure(self) -> None:
        bad_events = [
            {
                "event_id": "evt-good",
                "case_id": "case-err",
                "title": "Valid Event",
                "timestamp": "2026-11-10T10:00:00Z",
                "timestamp_precision": "EXACT_SECOND",
                "involved_entity_ids": [],
                "source_evidence_id": "ev-1",
            },
            {
                "event_id": "evt-corrupt-99",
                "case_id": "case-err",
                "title": "Corrupt Timestamp Event",
                "timestamp": "invalid-timestamp-value",
                "timestamp_precision": "EXACT_SECOND",
                "involved_entity_ids": [],
                "source_evidence_id": "ev-1",
            },
        ]
        with pytest.raises(ValueError, match="evt-corrupt-99"):
            reconstruct_timeline(bad_events)
