"""Reconstruction pipeline tests (synthetic warehouse burglary).

Most tests run with the PROPOSED decisions passed explicitly, so they exercise behaviour that is
conditional on approval. No decision is approved by default (see test_default_config_fails_closed).
"""

import json
import os
import random
from copy import deepcopy
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

import pytest
from tests.reconstruction.adapters.synthetic_burglary import (
    CASE_ID,
    EXPECTED_UTC,
    FFMPEG_AVAILABLE,
    SyntheticCase,
    build_case,
    evidence_path,
    snapshot_tree,
    storage_key,
)

from app.modules.reconstruction.adapters.contracts import (
    APPROVED_CONTRACT_DECISIONS,
    PROPOSED_CONTRACT_DECISIONS,
    ContractDecision,
)
from app.modules.reconstruction.adapters.diagnostics import DiagnosticSeverity
from app.modules.reconstruction.adapters.frame_handoff import FrameHandoff
from app.modules.reconstruction.pipeline import (
    ALGORITHM_VERSION,
    SNAPSHOT_SCHEMA_VERSION,
    FrameHandoffStatus,
    PipelineStatus,
    ReconstructionPipeline,
    ReconstructionPipelineConfig,
    ReconstructionResult,
    VideoStageStatus,
    event_candidates_from_snapshot,
)
from app.schemas.reconstruction import ContradictionLeadType, VideoExtractionOptions

ALL = PROPOSED_CONTRACT_DECISIONS
requires_ffmpeg = pytest.mark.skipif(not FFMPEG_AVAILABLE, reason="ffmpeg/ffprobe not installed")


def _config(
    case: SyntheticCase,
    *,
    approved: frozenset[ContractDecision] | None = None,
    with_video: bool = False,
    **overrides: Any,
) -> ReconstructionPipelineConfig:
    kwargs: dict[str, Any] = {
        "storage_root": case.storage_root,
        "artifact_root": case.artifact_root if with_video else None,
        "video_extraction_options": VideoExtractionOptions(
            interval_seconds=1.0, max_frames=5, create_unique_run_subdir=True
        ),
        **overrides,
    }
    kwargs["approved_contract_decisions"] = ALL if approved is None else approved
    return ReconstructionPipelineConfig(**kwargs)


def _run(
    case: SyntheticCase,
    *,
    candidates: Any = None,
    consumer: Any = None,
    run_id: str | None = None,
    **config: Any,
) -> ReconstructionResult:
    return ReconstructionPipeline(_config(case, **config), frame_consumer=consumer).run(
        case_id=CASE_ID,
        evidence=case.evidence,
        event_candidates=case.candidates if candidates is None else candidates,
        entities=case.entities,
        run_id=run_id,
    )


def _codes(result: ReconstructionResult, severity: DiagnosticSeverity | None = None) -> list[str]:
    return [d.code for d in result.diagnostics if severity is None or d.severity == severity]


def _utc(result: ReconstructionResult) -> dict[str, str]:
    return {
        e.event_id: e.timestamp_utc.isoformat().replace("+00:00", "Z")
        for e in result.timeline_events
    }


def _host_path_variants(*paths: Path) -> set[str]:
    out: set[str] = set()
    for path in paths:
        out |= {str(path), os.path.realpath(path), os.path.abspath(path)}
    return out


class RecordingFrameConsumer:
    """Test double for Member 1's interface: records handoffs and derives nothing."""

    def __init__(self) -> None:
        self.handoffs: list[FrameHandoff] = []

    def submit_frames(self, handoff: FrameHandoff) -> None:
        self.handoffs.append(handoff)


class FailingFrameConsumer:
    def submit_frames(self, handoff: FrameHandoff) -> None:
        raise ConnectionError("extraction service unreachable")


# --- Default config: fail closed --------------------------------------------------------------


def test_default_config_fails_closed(burglary_case: SyntheticCase) -> None:
    config = ReconstructionPipelineConfig(storage_root=burglary_case.storage_root)
    assert config.approved_contract_decisions == APPROVED_CONTRACT_DECISIONS == frozenset()
    result = ReconstructionPipeline(config).run(
        case_id=CASE_ID,
        evidence=burglary_case.evidence,
        event_candidates=burglary_case.candidates,
        entities=burglary_case.entities,
    )
    assert result.approved_contract_decisions == []
    assert result.withheld_contract_decisions == [f"DEC-00{i}" for i in range(1, 8)]
    assert result.timeline_events == []
    assert result.relationships == []
    assert len(result.excluded_inputs) == len(burglary_case.candidates)
    assert "CONTRACT_DECISION_PENDING" in _codes(result, DiagnosticSeverity.ERROR)


# --- Run conditional on the proposed decisions ------------------------------------------------


def test_end_to_end_burglary_reconstruction(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case)

    assert result.status == PipelineStatus.COMPLETED_WITH_WARNINGS
    assert result.approved_contract_decisions == [f"DEC-00{i}" for i in range(1, 8)]
    assert result.withheld_contract_decisions == []
    assert set(_codes(result, DiagnosticSeverity.WARNING)) == {
        "ENTITY_REFERENCE_UNRESOLVED",
        "VIDEO_STAGE_SKIPPED",
    }
    assert _codes(result, DiagnosticSeverity.ERROR) == []

    assert [e.event_id for e in result.timeline_events] == list(EXPECTED_UTC)
    assert _utc(result) == EXPECTED_UTC

    (lead,) = result.contradiction_leads
    assert lead.lead_type == ContradictionLeadType.SPEED_THRESHOLD_EXCEEDANCE_LEAD
    assert lead.entity_id == "ent-person-ravi"

    (rel,) = result.relationships
    assert rel.relation_type == "CONTRADICTS"
    assert rel.is_observed is False
    assert rel.weight is None
    assert rel.evidence_basis == ["ev-cctv-dock", "ev-chat-export"]
    assert lead.relationship_id == rel.relationship_id
    assert rel.metadata["provenance"]["origin_event"]["source_record_id"] == "osd:2026-11-10T21:42:00"
    assert rel.metadata["provenance"]["origin_event"]["raw_timestamp"] == "2026-11-10T21:42:00"

    lead_dump = lead.model_dump()
    assert "relationship" not in lead_dump  # legacy engine DTO is not part of the output


def test_timeline_events_carry_raw_and_corrected_time(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case)
    events = {e.event_id: e for e in result.timeline_events}

    cctv = events["evt-01-shutter-forced"]
    assert cctv.raw_timestamp == "2026-11-10T21:42:00"
    assert cctv.timestamp_utc == datetime(2026, 11, 10, 16, 10, tzinfo=UTC)
    assert cctv.source_timezone == "Asia/Kolkata"
    assert cctv.clock_offset_seconds == -120.0
    assert cctv.clock_calibration == "DECLARED_NOT_INDEPENDENTLY_VERIFIED"
    assert cctv.timezone_basis == "SOURCE_TIMEZONE_METADATA"
    assert cctv.timestamp_precision == "EXACT_SECOND"
    assert cctv.uncertainty_window_start == cctv.timestamp_utc
    assert cctv.uncertainty_window_end == cctv.timestamp_utc + timedelta(seconds=1)
    assert cctv.duration_seconds == 30.0

    cdr = events["evt-02-outbound-call"]
    assert cdr.source_timezone is None
    assert cdr.clock_offset_seconds is None  # not declared → null, not 0.0
    assert cdr.clock_calibration == "NOT_DECLARED_UNCALIBRATED"
    assert cdr.timezone_basis == "TIMESTAMP_UTC_OFFSET"
    assert cdr.duration_seconds is None

    clock_info = [
        d.subject_id for d in result.diagnostics if d.code == "CLOCK_OFFSET_NOT_DECLARED"
    ]
    assert clock_info == ["ev-cdr-tower"]

    msg = events["evt-04-vehicle-message"]
    assert msg.involved_entity_ids == ["ent-person-ravi"]
    assert msg.unresolved_entity_raw_ids == ["ent-unknown-77"]
    assert msg.extraction_confidence is None


def test_unified_extraction_json_input_matches_list_input(burglary_case: SyntheticCase) -> None:
    from_list = _run(burglary_case, run_id="fixed").to_snapshot()
    from_json = _run(burglary_case, candidates=burglary_case.unified_extraction_json(), run_id="fixed")
    assert from_json.to_snapshot() == from_list


def test_spatial_observations_are_extracted_coordinates(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case)
    pairs = {(o.entity_id, o.event_id) for o in result.spatial_observations}
    assert ("ent-person-ravi", "evt-01-shutter-forced") in pairs
    assert ("ent-person-ravi", "evt-03-location-shared") in pairs
    assert ("ent-person-ravi", "evt-04-vehicle-message") not in pairs  # no coordinates
    assert all(o.is_observed for o in result.spatial_observations)


# --- Entity handling ---------------------------------------------------------------------


def test_alias_references_reach_spatial_as_canonical_ids(burglary_case: SyntheticCase) -> None:
    burglary_case.candidate_by_id("evt-03-location-shared")["involved_entity_raw_ids"] = ["R. Kumar"]
    result = _run(burglary_case)
    assert result.spatial is not None
    assert "evt-03-location-shared" in result.spatial.trajectories_by_entity["ent-person-ravi"].located_event_ids
    assert len(result.contradiction_leads) == 1


def test_similar_name_reference_does_not_create_lead(burglary_case: SyntheticCase) -> None:
    burglary_case.candidate_by_id("evt-03-location-shared")["involved_entity_raw_ids"] = ["Ravi Kumaar"]
    result = _run(burglary_case)
    assert result.contradiction_leads == []
    assert result.relationships == []


# --- Double-correction guard ------------------------------------------------------------------


def test_corrected_time_equals_raw_plus_offset_exactly_once(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case)
    for event in result.timeline_events:
        if event.event_id not in ("evt-01-shutter-forced", "evt-05-van-departs"):
            continue
        raw_local = datetime.fromisoformat(event.raw_timestamp)
        raw_utc = raw_local.replace(tzinfo=None) - timedelta(hours=5, minutes=30)
        assert event.timestamp_utc.replace(tzinfo=None) == raw_utc + timedelta(seconds=-120)


def test_rerun_on_snapshot_candidates_does_not_correct_twice(burglary_case: SyntheticCase) -> None:
    first = _run(burglary_case)
    snapshot = json.loads(first.to_snapshot_json())
    candidates = event_candidates_from_snapshot(snapshot)
    assert candidates == first.to_event_candidates()
    assert {c["raw_timestamp"] for c in candidates} == {
        c["raw_timestamp"] for c in burglary_case.candidates
    }

    second = _run(burglary_case, candidates=candidates)
    third = _run(burglary_case, candidates=second.to_event_candidates())
    assert _utc(first) == _utc(second) == _utc(third) == EXPECTED_UTC
    assert [e.raw_timestamp for e in third.timeline_events] == [
        e.raw_timestamp for e in first.timeline_events
    ]


def test_feeding_full_output_events_back_ignores_derived_time(burglary_case: SyntheticCase) -> None:
    first = _run(burglary_case)
    output_events = first.to_snapshot()["timeline_events"]
    assert "timestamp_utc" in output_events[0]

    second = _run(burglary_case, candidates=output_events)
    assert _utc(second) == EXPECTED_UTC
    ignored = [d for d in second.diagnostics if d.code == "DERIVED_FIELDS_IGNORED"]
    assert len(ignored) == len(EXPECTED_UTC)
    assert second.status == PipelineStatus.COMPLETED_WITH_WARNINGS


def test_already_utc_raw_timestamp_not_localized_again(burglary_case: SyntheticCase) -> None:
    # A DVR that writes Z timestamps: the instant is taken as given and only the clock offset
    # (declared once on the evidence) is applied.
    burglary_case.candidate_by_id("evt-01-shutter-forced")["raw_timestamp"] = "2026-11-10T16:12:00Z"
    result = _run(burglary_case)
    assert _utc(result)["evt-01-shutter-forced"] == "2026-11-10T16:10:00Z"


# --- Snapshot / persistence readiness ------------------------------------------------------


def test_snapshot_is_complete_json(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case)
    snapshot = json.loads(result.to_snapshot_json())
    for key in (
        "snapshot_schema_version",
        "algorithm_version",
        "configuration_version",
        "configuration",
        "run_id",
        "case_id",
        "status",
        "evidence",
        "timeline_events",
        "timeline_overlaps",
        "spatial_observations",
        "relationships",
        "contradiction_leads",
        "video",
        "diagnostics",
    ):
        assert key in snapshot
    assert snapshot["snapshot_schema_version"] == SNAPSHOT_SCHEMA_VERSION
    assert snapshot["algorithm_version"] == ALGORITHM_VERSION
    assert snapshot["case_id"] == CASE_ID
    assert snapshot["configuration"]["approved_contract_decisions"] == [
        f"DEC-00{i}" for i in range(1, 8)
    ]
    assert snapshot["relationships"][0]["weight"] is None
    assert snapshot["timeline_events"][0]["timestamp_utc"].endswith("Z")
    assert ReconstructionResult.model_validate_json(result.to_snapshot_json()) == result


def test_snapshot_contains_no_host_paths(burglary_case: SyntheticCase) -> None:
    text = _run(burglary_case, with_video=True).to_snapshot_json()
    for variant in _host_path_variants(burglary_case.storage_root, burglary_case.artifact_root):
        assert variant not in text
    evidence = json.loads(text)["evidence"]
    assert {e["evidence_id"]: e["storage_reference"] for e in evidence} == {
        eid: f"urn:yukti:evidence:{eid}:storage"
        for eid in ("ev-cctv-dock", "ev-cdr-tower", "ev-chat-export")
    }
    assert {e["storage_key"] for e in evidence} == {
        storage_key(eid) for eid in ("ev-cctv-dock", "ev-cdr-tower", "ev-chat-export")
    }


def test_run_id_and_configuration_version(burglary_case: SyntheticCase) -> None:
    first = _run(burglary_case)
    again = _run(burglary_case)
    assert first.run_id == again.run_id and first.run_id.startswith("run-")
    assert first.configuration_version.startswith("cfg-sha256-")

    explicit = _run(burglary_case, run_id="run-member2-0001")
    assert explicit.run_id == "run-member2-0001"

    changed_config = _run(burglary_case, speed_threshold_kmh=90.0)
    assert changed_config.configuration_version != first.configuration_version
    assert changed_config.run_id != first.run_id

    burglary_case.candidate_by_id("evt-02-outbound-call")["description"] = "edited"
    assert _run(burglary_case).run_id != first.run_id


def test_output_deterministic_across_input_order(burglary_case: SyntheticCase) -> None:
    first = _run(burglary_case).to_snapshot()
    shuffled = deepcopy(burglary_case)
    rng = random.Random(20261110)
    for seq in (shuffled.evidence, shuffled.candidates, shuffled.entities):
        rng.shuffle(seq)
    assert _run(shuffled).to_snapshot() == first


# --- Failure handling --------------------------------------------------------------------


def test_bad_evidence_is_excluded_and_run_continues(burglary_case: SyntheticCase) -> None:
    burglary_case.evidence_by_id("ev-cdr-tower")["sha256_hash"] = "f" * 64
    result = _run(burglary_case)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    err = next(d for d in result.diagnostics if d.code == "EVIDENCE_INTEGRITY_MISMATCH")
    assert err.subject_id == "ev-cdr-tower"
    assert {(x.subject_type, x.subject_id, x.reason_code) for x in result.excluded_inputs} == {
        ("EVIDENCE", "ev-cdr-tower", "EVIDENCE_INTEGRITY_MISMATCH"),
        ("EVENT_CANDIDATE", "evt-02-outbound-call", "SOURCE_EVIDENCE_EXCLUDED"),
    }
    record = next(e for e in result.evidence if e.evidence_id == "ev-cdr-tower")
    assert record.usable is False and record.integrity_verified is False
    # The rest of the case is still reconstructed, including the contradiction.
    assert [e.event_id for e in result.timeline_events] == [
        k for k in EXPECTED_UTC if k != "evt-02-outbound-call"
    ]
    assert len(result.relationships) == 1
    assert any(lim.code == "EXCLUDED_INPUTS" for lim in result.limitations)
    assert {(g.gap_type, g.subject_id) for g in result.evidence_gaps} >= {
        ("EVIDENCE_EXCLUDED", "ev-cdr-tower"),
        ("CANDIDATE_EXCLUDED", "evt-02-outbound-call"),
    }


def test_bad_evidence_gets_no_video_side_effects(burglary_case: SyntheticCase) -> None:
    burglary_case.evidence_by_id("ev-cctv-dock")["sha256_hash"] = "f" * 64
    result = _run(burglary_case, with_video=True)
    assert result.video == []
    assert list(burglary_case.artifact_root.iterdir()) == []
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS


def test_all_validation_errors_reported_together(burglary_case: SyntheticCase) -> None:
    burglary_case.evidence_by_id("ev-cdr-tower")["storage_uri"] = "s3://bucket/cdr.csv"
    burglary_case.evidence.append(
        {**deepcopy(burglary_case.evidence_by_id("ev-chat-export")), "evidence_id": "ev-extra",
         "storage_uri": "/srv/evidence/chat.txt"}
    )
    burglary_case.candidate_by_id("evt-03-location-shared")["raw_timestamp"] = "2026-11-10T21:44:00"
    del burglary_case.evidence_by_id("ev-chat-export")["metadata"]["source_timezone"]
    burglary_case.candidate_by_id("evt-05-van-departs")["coordinates"] = {"lat": 999, "lon": 0}
    burglary_case.candidates.append(deepcopy(burglary_case.candidate_by_id("evt-01-shutter-forced")))

    result = _run(burglary_case)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    errors = {(d.code, d.subject_id) for d in result.diagnostics if d.severity == DiagnosticSeverity.ERROR}
    assert ("REMOTE_STORAGE_URI_REJECTED", "ev-cdr-tower") in errors
    assert ("CLIENT_STORAGE_PATH_REJECTED", "ev-extra") in errors
    assert ("SOURCE_TIMEZONE_UNKNOWN", "evt-03-location-shared") in errors
    assert ("INVALID_EVENT_CANDIDATE", "evt-05-van-departs") in errors
    assert ("DUPLICATE_CANDIDATE_ID", "evt-01-shutter-forced") in errors
    remote = next(d for d in result.diagnostics if d.code == "REMOTE_STORAGE_URI_REJECTED")
    assert "staging is not implemented" in remote.message
    assert [e.event_id for e in result.timeline_events] == []  # every remaining candidate excluded


def test_malformed_candidate_payload_fails(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case, candidates='{"event_candidates": [')
    assert result.status == PipelineStatus.FAILED
    assert "INVALID_EVENT_CANDIDATE_PAYLOAD" in _codes(result, DiagnosticSeverity.ERROR)


def test_non_object_candidate_is_reported_not_dropped(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case, candidates=[*burglary_case.candidates, "not-an-object"])
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert len(result.timeline_events) == 5
    bad = next(d for d in result.diagnostics if d.code == "INVALID_EVENT_CANDIDATE")
    assert bad.subject_id == "<index_5>"


def test_invalid_entity_registry_fails(burglary_case: SyntheticCase) -> None:
    burglary_case.entities[0]["case_id"] = "case-other"
    result = _run(burglary_case)
    assert result.status == PipelineStatus.FAILED
    assert "INVALID_ENTITY_REGISTRY" in _codes(result, DiagnosticSeverity.ERROR)


def test_invalid_top_level_inputs_raise(burglary_case: SyntheticCase) -> None:
    pipeline = ReconstructionPipeline(ReconstructionPipelineConfig(storage_root=burglary_case.storage_root))
    with pytest.raises(ValueError, match="case_id"):
        pipeline.run(case_id=" ", evidence=[], event_candidates=[], entities=[])
    with pytest.raises(TypeError, match="sequence"):
        pipeline.run(case_id=CASE_ID, evidence="ev-1", event_candidates=[], entities=[])
    with pytest.raises(ValueError, match="run_id"):
        pipeline.run(case_id=CASE_ID, evidence=[], event_candidates=[], entities=[], run_id=" ")


# --- Withheld decisions (gate still works) ---------------------------------------------------


def test_withheld_relationship_decisions_keep_leads_separate(burglary_case: SyntheticCase) -> None:
    approved = ALL - {ContractDecision.RELATION_TYPE_TAXONOMY}
    result = _run(burglary_case, approved=approved)
    assert result.relationships == []
    (lead,) = result.contradiction_leads
    assert lead.relationship_id is None
    blocked = next(d for d in result.diagnostics if d.code == "RELATIONSHIP_EXPORT_BLOCKED")
    assert blocked.pending_contract_decisions == ["DEC-003"]
    assert result.withheld_contract_decisions == ["DEC-003"]


def test_withheld_dec007_refuses_utc_instead_of_assuming_it(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case, approved=ALL - {ContractDecision.RAW_TIMESTAMP_PRESERVATION})
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert result.timeline_events == []
    pending = [d for d in result.diagnostics if d.code == "CONTRACT_DECISION_PENDING"]
    assert len(pending) == len(EXPECTED_UTC)
    assert all(d.pending_contract_decisions == ["DEC-007"] for d in pending)


# --- Video: frame extraction with provenance only ---------------------------------------------


def test_corrupt_cctv_bytes_are_non_fatal_and_paths_scrubbed(burglary_case: SyntheticCase) -> None:
    result = _run(burglary_case, with_video=True)
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    (video,) = result.video
    assert video.status == VideoStageStatus.FAILED
    assert video.frame_artifact_ids == []
    failure = next(d for d in result.diagnostics if d.code == "VIDEO_PROCESSING_FAILED")
    for variant in _host_path_variants(burglary_case.storage_root, burglary_case.artifact_root):
        assert variant not in failure.message
    assert len(result.timeline_events) == 5


def test_inputs_and_evidence_never_mutated(burglary_case: SyntheticCase) -> None:
    before_tree = snapshot_tree(burglary_case.storage_root)
    before_inputs = deepcopy((burglary_case.evidence, burglary_case.candidates, burglary_case.entities))
    _run(burglary_case, with_video=True)
    assert snapshot_tree(burglary_case.storage_root) == before_tree
    assert (burglary_case.evidence, burglary_case.candidates, burglary_case.entities) == before_inputs


def test_estimated_precision_marked_as_assumption(burglary_case: SyntheticCase) -> None:
    burglary_case.candidate_by_id("evt-04-vehicle-message")["timestamp_precision"] = "ESTIMATED"
    result = _run(burglary_case)
    event = next(e for e in result.timeline_events if e.event_id == "evt-04-vehicle-message")
    assert event.is_estimated_window_assumption is True
    assert event.uncertainty_window_end - event.uncertainty_window_start == timedelta(seconds=1800)  # type: ignore[operator]


@requires_ffmpeg
def test_frames_extracted_and_not_submitted_without_consumer(tmp_path: Path) -> None:
    case = build_case(tmp_path, real_video=True)
    before = snapshot_tree(case.storage_root)
    result = _run(case, with_video=True)

    (video,) = result.video
    assert video.status == VideoStageStatus.EXTRACTED
    assert video.processing_kind == "FRAME_EXTRACTION_WITH_PROVENANCE"
    assert "No semantic event recognition" in video.processing_note
    assert video.frame_handoff_status == FrameHandoffStatus.NOT_SUBMITTED_NO_CONSUMER
    assert video.frames_extracted >= 1 and video.source_integrity_verified is True
    assert "FRAME_CONSUMER_UNAVAILABLE" in _codes(result, DiagnosticSeverity.INFO)

    sha = case.evidence_by_id("ev-cctv-dock")["sha256_hash"]
    for frame in result.frame_artifacts:
        assert frame.source_video_sha256 == sha
        assert frame.artifact_reference == f"urn:yukti:artifact:{frame.artifact_id}"
        assert frame.artifact_relative_path is not None
        assert not frame.artifact_relative_path.startswith("/")
        assert (case.artifact_root / frame.artifact_relative_path).is_file()

    assert len(result.timeline_events) == 5  # frames never become events
    assert snapshot_tree(case.storage_root) == before
    text = result.to_snapshot_json()
    for variant in _host_path_variants(case.storage_root, case.artifact_root):
        assert variant not in text


@requires_ffmpeg
def test_frames_anchored_once_and_handed_off(tmp_path: Path) -> None:
    case = build_case(tmp_path, real_video=True)
    consumer = RecordingFrameConsumer()
    result = _run(case, with_video=True, consumer=consumer)

    (video,) = result.video
    assert video.frame_handoff_status == FrameHandoffStatus.SUBMITTED
    assert video.timestamps_utc_anchored is True
    (handoff,) = consumer.handoffs
    assert handoff.source_video_sha256 == case.evidence_by_id("ev-cctv-dock")["sha256_hash"]
    first = result.frame_artifacts[0]
    # recording start 21:30:00 IST = 16:00:00Z, DVR 120 s fast → 15:58:00Z + PTS (applied once).
    expected = datetime(2026, 11, 10, 15, 58, tzinfo=UTC) + timedelta(seconds=first.pts_offset_seconds)
    assert first.timestamp_utc == expected


@requires_ffmpeg
def test_frame_consumer_failure_is_recorded(tmp_path: Path) -> None:
    case = build_case(tmp_path, real_video=True)
    result = _run(case, with_video=True, consumer=FailingFrameConsumer())
    (video,) = result.video
    assert video.frame_handoff_status == FrameHandoffStatus.FAILED
    failure = next(d for d in result.diagnostics if d.code == "FRAME_HANDOFF_FAILED")
    assert "ConnectionError" in failure.message
    assert result.status == PipelineStatus.COMPLETED_WITH_ERRORS
    assert len(result.relationships) == 1


@requires_ffmpeg
def test_naive_recording_start_without_timezone_not_anchored(tmp_path: Path) -> None:
    case = build_case(tmp_path, real_video=True)
    del case.evidence_by_id("ev-cctv-dock")["metadata"]["source_timezone"]
    # CCTV candidates are naive too; give them explicit offsets so they remain valid.
    for cid in ("evt-01-shutter-forced", "evt-05-van-departs"):
        case.candidate_by_id(cid)["raw_timestamp"] += "+05:30"
    result = _run(case, with_video=True)
    (video,) = result.video
    assert video.timestamps_utc_anchored is False
    assert all(f.timestamp_utc is None for f in result.frame_artifacts)
    assert "FRAME_UTC_ANCHORING_WITHHELD" in _codes(result, DiagnosticSeverity.WARNING)
    assert evidence_path(case.storage_root, "ev-cctv-dock").is_file()
