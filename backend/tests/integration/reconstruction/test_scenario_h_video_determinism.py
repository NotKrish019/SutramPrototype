"""Phase 3D.2: real-video determinism.

The video stage runs real ffmpeg on a synthetic MP4, repeatedly. Stable output fields and frame
hashes must be identical across runs. Only the per-run artifact directory differs; it is excluded
from ``result_sha256`` by design, because derived artifacts are never overwritten.
"""

import hashlib
from pathlib import Path
from typing import Any

import pytest

from app.modules.reconstruction.pipeline import ReconstructionResult, VideoStageStatus
from tests.integration.reconstruction.support import FFMPEG_AVAILABLE, build_case, run_pipeline

requires_ffmpeg = pytest.mark.skipif(not FFMPEG_AVAILABLE, reason="ffmpeg/ffprobe not installed")

STABLE_FRAME_FIELDS = (
    "artifact_id",
    "artifact_reference",
    "origin",
    "case_id",
    "source_evidence_id",
    "source_video_sha256",
    "frame_index",
    "pts_offset_seconds",
    "timestamp_utc_anchored",
    "timestamp_utc",
    "timestamp_precision",
    "time_window",
    "sha256_hash",
    "file_size_bytes",
    "image_format",
    "width",
    "height",
    "extraction_method",
)


def _stable(result: ReconstructionResult) -> dict[str, Any]:
    snapshot = result.to_snapshot()
    return {
        "result_sha256": result.result_sha256,
        "video": snapshot["video"],
        "frames": [{k: f[k] for k in STABLE_FRAME_FIELDS} for f in snapshot["frame_artifacts"]],
        "timeline": snapshot["timeline_events"],
    }


@requires_ffmpeg
def test_real_video_runs_are_deterministic_across_fresh_roots(tmp_path: Path) -> None:
    outputs: list[dict[str, Any]] = []
    for i in range(3):
        root = tmp_path / f"run{i}"
        root.mkdir()
        result = run_pipeline(build_case(root, real_video=True), with_video=True)
        (video,) = result.video
        assert video.status == VideoStageStatus.EXTRACTED
        assert video.source_integrity_verified is True
        assert video.source_video_sha256_before == video.source_video_sha256_after
        assert result.frame_artifacts, "real ffmpeg extraction produced no frames"
        assert all(f.timestamp_utc_anchored for f in result.frame_artifacts)
        outputs.append(_stable(result))
    assert outputs[0] == outputs[1] == outputs[2]


@requires_ffmpeg
def test_repeated_extraction_into_one_artifact_root_is_stable_and_never_overwrites(tmp_path: Path) -> None:
    case = build_case(tmp_path, real_video=True)
    first = run_pipeline(case, with_video=True)
    first_files = {
        f.artifact_relative_path: hashlib.sha256((case.artifact_root / str(f.artifact_relative_path)).read_bytes()).hexdigest()
        for f in first.frame_artifacts
    }
    second = run_pipeline(case, with_video=True)

    assert _stable(first) == _stable(second)
    first_paths = {f.artifact_relative_path for f in first.frame_artifacts}
    second_paths = {f.artifact_relative_path for f in second.frame_artifacts}
    assert first_paths.isdisjoint(second_paths)  # new run directory, nothing overwritten
    for rel, digest in first_files.items():
        assert hashlib.sha256((case.artifact_root / str(rel)).read_bytes()).hexdigest() == digest
    # The hash recorded for each frame is the hash of the file on disk.
    for frame in second.frame_artifacts:
        on_disk = (case.artifact_root / str(frame.artifact_relative_path)).read_bytes()
        assert hashlib.sha256(on_disk).hexdigest() == frame.sha256_hash
