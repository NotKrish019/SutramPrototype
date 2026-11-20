"""Scenario H: video frame extraction with provenance; no consumer; never semantic recognition."""

import hashlib
import re
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from app.modules.reconstruction.adapters.frame_handoff import FrameHandoff
from app.modules.reconstruction.pipeline import FrameHandoffStatus, VideoStageStatus
from app.modules.reporting.forensic_report import build_forensic_report
from app.modules.reporting.report_render import render_html, render_markdown
from tests.integration.reconstruction.support import (
    BASE,
    FFMPEG_AVAILABLE,
    FIXED_NOW,
    SyntheticCase,
    build_case,
    get_view,
    make_service,
    post_run,
    run_pipeline,
    synthetic_candidate,
    use_service,
)

requires_ffmpeg = pytest.mark.skipif(not FFMPEG_AVAILABLE, reason="ffmpeg/ffprobe not installed")

# Words that would claim the frames were understood. Allowed only inside explicit negations.
RECOGNITION_CLAIM = re.compile(r"\b(recogni[sz]ed|detected|identified|shows)\b", re.IGNORECASE)


class RecordingConsumer:
    def __init__(self) -> None:
        self.handoffs: list[FrameHandoff] = []

    def submit_frames(self, handoff: FrameHandoff) -> None:
        self.handoffs.append(handoff)


@pytest.fixture
def video_case(tmp_path: Path) -> SyntheticCase:
    case = build_case(tmp_path, real_video=True)
    # One CCTV event inside the 2 s recording (21:30:00 IST, DVR 120 s fast → 15:58:00Z).
    case.candidates.append(
        synthetic_candidate("evt-00-dock-motion", "ev-cctv-dock", "2026-11-10T21:30:01", description="Motion at dock")
    )
    return case


@requires_ffmpeg
def test_frames_are_extracted_with_provenance_and_not_submitted(video_case: SyntheticCase) -> None:
    result = run_pipeline(video_case, with_video=True)
    (video,) = result.video
    assert video.status == VideoStageStatus.EXTRACTED
    assert video.frame_handoff_status == FrameHandoffStatus.NOT_SUBMITTED_NO_CONSUMER
    assert video.processing_kind == "FRAME_EXTRACTION_WITH_PROVENANCE"
    assert "No semantic event recognition was performed" in video.processing_note
    assert video.frames_extracted == len(result.frame_artifacts) > 0
    assert video.source_video_sha256_before == video.source_video_sha256_after
    assert video.source_integrity_verified is True

    cctv_sha = next(e.sha256_hash for e in result.evidence if e.evidence_id == "ev-cctv-dock")
    for frame in result.frame_artifacts:
        assert frame.source_video_sha256 == cctv_sha
        assert frame.artifact_reference == f"urn:yukti:artifact:{frame.artifact_id}"
        assert frame.artifact_relative_path is not None and not Path(frame.artifact_relative_path).is_absolute()
        data = (video_case.artifact_root / frame.artifact_relative_path).read_bytes()
        assert hashlib.sha256(data).hexdigest() == frame.sha256_hash
    assert any(d.code == "FRAME_CONSUMER_UNAVAILABLE" for d in result.diagnostics)
    assert "FRAMES_NOT_SEMANTICALLY_ANALYZED" in {lim.code for lim in result.limitations}


@requires_ffmpeg
def test_frames_never_become_events(video_case: SyntheticCase) -> None:
    result = run_pipeline(video_case, with_video=True)
    candidate_ids = {c["candidate_id"] for c in video_case.candidates}
    assert {e.event_id for e in result.timeline_events} == candidate_ids
    frame_ids = {f.artifact_id for f in result.frame_artifacts}
    assert not frame_ids & {e.event_id for e in result.timeline_events}

    # A frame inside an event's window is linked as temporal alignment only.
    motion = next(e for e in result.timeline_events if e.event_id == "evt-00-dock-motion")
    assert motion.supporting_frame_artifact_ids
    report = build_forensic_report(result, generated_at=FIXED_NOW)
    entry = next(t for t in report.timeline if t.event_id == "evt-00-dock-motion")
    assert entry.frame_artifact_ids == motion.supporting_frame_artifact_ids
    assert entry.citations[0].frame_artifact_ids == motion.supporting_frame_artifact_ids
    markdown = render_markdown(report)
    assert "Frames captured during this window: `" in markdown
    assert "They do not show or confirm the event." in markdown


@requires_ffmpeg
def test_with_consumer_frames_are_submitted_but_nothing_is_derived(video_case: SyntheticCase) -> None:
    consumer = RecordingConsumer()
    with_consumer = run_pipeline(video_case, with_video=True, consumer=consumer)
    without = run_pipeline(video_case, with_video=True)
    assert with_consumer.video[0].frame_handoff_status == FrameHandoffStatus.SUBMITTED
    assert len(consumer.handoffs) == 1 and consumer.handoffs[0].frames
    assert [e.event_id for e in with_consumer.timeline_events] == [e.event_id for e in without.timeline_events]


@requires_ffmpeg
def test_report_never_claims_semantic_recognition(video_case: SyntheticCase) -> None:
    result = run_pipeline(video_case, with_video=True)
    report = build_forensic_report(result, generated_at=FIXED_NOW)
    pending = {(p.code, p.subject_id) for p in report.limitations.pending_items}
    assert ("FRAMES_NOT_SUBMITTED_NO_CONSUMER", "ev-cctv-dock") in pending
    for text in (render_markdown(report), render_html(report), report.to_json()):
        for match in RECOGNITION_CLAIM.finditer(text):
            window = text[max(0, match.start() - 40) : match.end()].lower()
            assert re.search(r"\b(no|not|never|nor)\b", window), window


@requires_ffmpeg
def test_video_through_api_reports_no_consumer(client: TestClient, video_case: SyntheticCase) -> None:
    use_service(make_service(video_case, with_video=True))
    run_id = post_run(client, video_case)["run_id"]
    (video,) = get_view(client, run_id, "/diagnostics")["video"]
    assert video["status"] == "EXTRACTED"
    assert video["frame_handoff_status"] == "NOT_SUBMITTED_NO_CONSUMER"
    html = client.get(f"{BASE}/runs/{run_id}/report", params={"format": "html"}).text
    assert "FRAMES_NOT_SUBMITTED_NO_CONSUMER" in html
    for root in (str(video_case.artifact_root), str(video_case.storage_root)):
        assert root not in html


def test_without_artifact_root_video_is_skipped_and_reported(burglary_case: SyntheticCase) -> None:
    result = run_pipeline(burglary_case)
    (video,) = result.video
    assert video.status == VideoStageStatus.SKIPPED_NO_ARTIFACT_ROOT
    assert video.frame_handoff_status is None
    report = build_forensic_report(result, generated_at=FIXED_NOW)
    assert ("VIDEO_FRAMES_NOT_EXTRACTED", "ev-cctv-dock") in {
        (p.code, p.subject_id) for p in report.limitations.pending_items
    }


@requires_ffmpeg
def test_corrupt_video_fails_the_video_stage_only(tmp_path: Path) -> None:
    case = build_case(tmp_path)  # placeholder bytes: valid hash, not a decodable video
    result = run_pipeline(case, with_video=True)
    assert result.video[0].status == VideoStageStatus.FAILED
    assert len(result.timeline_events) == 5
    assert any(d.code == "VIDEO_PROCESSING_FAILED" for d in result.diagnostics)
