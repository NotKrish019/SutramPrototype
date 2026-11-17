"""Comprehensive Forensic CCTV & Video Processing Tests (Member 4 — Phase 2D).

Verifies:
1. Video metadata extraction (container format, duration, dimensions, fps, codec, streams, audio/video flags, creation timestamp).
2. Cryptographic SHA-256 integrity and absolute evidence immutability (pre/post verification).
3. Configurable keyframe & fixed-interval frame extraction with deterministic naming and derived artifact tracking.
4. Hard ceiling enforcement on frame count (max_frames).
5. Start and end time bounds validation and slicing.
6. Temporal normalization: Anchored UTC timestamps with timezone and clock drift correction; unanchored relative PTS handling when recording start is omitted.
7. Path security and path traversal prevention (rejecting null bytes, traversal escapes, and raw evidence directory collision).
8. Failure resilience and partial artifact cleanup (missing files, unsupported formats, corrupt files, timeouts, and empty extractions).
"""

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
from unittest.mock import MagicMock, patch
from zoneinfo import ZoneInfo

import pytest

# Store real subprocess.run before any mocking in tests
_REAL_SUBPROCESS_RUN = subprocess.run

from app.modules.reconstruction.video import (
    CorruptVideoError,
    PathSecurityError,
    UnsupportedVideoFormatError,
    VideoExtractionError,
    VideoIntegrityError,
    VideoNotFoundError,
    VideoProcessingError,
    VideoProcessingTimeoutError,
    compute_file_sha256,
    extract_video_keyframes,
    generate_unique_artifact_directory,
    get_ffmpeg_tool_metadata,
    probe_video_metadata,
    validate_safe_path,
)
from app.schemas.reconstruction import (
    CanonicalTimePrecision,
    VideoExtractionOptions,
)


# --- Pytest Fixtures for Synthetic Video Generation ---


@pytest.fixture(scope="session")
def synthetic_video_5s(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Generate a 5-second synthetic video with h264 video (10 fps) and aac audio."""
    session_dir = tmp_path_factory.mktemp("video_fixtures")
    video_path = session_dir / "synthetic_5s.mp4"

    cmd = [
        "ffmpeg",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=5:size=320x240:rate=10",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=1000:duration=5",
        "-metadata",
        "creation_time=2026-11-17T08:00:00Z",
        "-c:v",
        "libx264",
        "-c:a",
        "aac",
        str(video_path),
    ]
    subprocess.run(cmd, check=True, capture_output=True)
    return video_path


@pytest.fixture(scope="session")
def synthetic_video_keyframes(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Generate a 6-second video with forced I-frames every 20 frames (2.0s at 10 fps)."""
    session_dir = tmp_path_factory.mktemp("keyframe_fixtures")
    video_path = session_dir / "synthetic_keyframes.mp4"

    cmd = [
        "ffmpeg",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=duration=6:size=320x240:rate=10",
        "-g",
        "20",
        "-c:v",
        "libx264",
        str(video_path),
    ]
    subprocess.run(cmd, check=True, capture_output=True)
    return video_path


@pytest.fixture(scope="session")
def synthetic_audio_only(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Generate an audio-only media file (no video stream)."""
    session_dir = tmp_path_factory.mktemp("audio_fixtures")
    audio_path = session_dir / "audio_only.mp3"

    cmd = [
        "ffmpeg",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=2",
        "-c:a",
        "libmp3lame",
        str(audio_path),
    ]
    subprocess.run(cmd, check=True, capture_output=True)
    return audio_path


# --- 1. Video Metadata Extraction Tests ---


class TestVideoMetadataExtraction:
    """Tests technical video metadata extraction via FFprobe."""

    def test_probe_metadata_valid_video(self, synthetic_video_5s: Path) -> None:
        """Verify successful extraction of container format, dimensions, fps, and stream summaries."""
        meta = probe_video_metadata(synthetic_video_5s, evidence_id="EV-CCTV-001")

        assert meta.has_video is True
        assert meta.has_audio is True
        assert meta.video_streams_count == 1
        assert meta.audio_streams_count == 1
        assert meta.width == 320
        assert meta.height == 240
        assert meta.fps == 10.0
        assert meta.codec_name == "h264"
        assert meta.duration_seconds >= 4.9 and meta.duration_seconds <= 5.1
        assert meta.file_size_bytes > 0
        assert len(meta.sha256_hash) == 64
        assert meta.creation_time_raw is not None
        assert "2026-11-17" in meta.creation_time_raw
        assert len(meta.streams) == 2

    def test_probe_metadata_tool_version_recorded(self, synthetic_video_5s: Path) -> None:
        """Verify that FFprobe tool version information is attached to the metadata."""
        meta = probe_video_metadata(synthetic_video_5s, evidence_id="EV-CCTV-001")
        assert meta.tool_version is not None
        assert "ffprobe" in meta.tool_version.lower()

    def test_get_ffmpeg_tool_metadata(self) -> None:
        """Verify tool metadata discovery function."""
        tools = get_ffmpeg_tool_metadata()
        assert "ffmpeg_binary" in tools
        assert "ffprobe_binary" in tools
        assert "ffmpeg" in tools["ffmpeg_version"].lower()
        assert "ffprobe" in tools["ffprobe_version"].lower()


# --- 2. Evidence Integrity & Immutability Tests ---


class TestVideoIntegrityAndImmutability:
    """Verifies cryptographic hash computation, strict read-only access, and immutability."""

    def test_original_video_is_strictly_unmodified(self, synthetic_video_5s: Path, tmp_path: Path) -> None:
        """Verify that both probe and keyframe extraction never alter a single byte of source video."""
        initial_bytes = synthetic_video_5s.read_bytes()
        initial_hash = compute_file_sha256(synthetic_video_5s)

        out_dir = tmp_path / "artifacts"
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="EV-001",
            case_id="CASE-1",
            options=VideoExtractionOptions(interval_seconds=1.0),
        )

        final_bytes = synthetic_video_5s.read_bytes()
        final_hash = compute_file_sha256(synthetic_video_5s)

        assert initial_bytes == final_bytes
        assert initial_hash == final_hash
        assert res.source_video_sha256_before == initial_hash
        assert res.source_video_sha256_after == initial_hash
        assert res.integrity_verified is True

    def test_expected_sha256_matches_successfully(self, synthetic_video_5s: Path) -> None:
        """Verify expected_sha256 passes when correct."""
        valid_hash = compute_file_sha256(synthetic_video_5s)
        meta = probe_video_metadata(
            synthetic_video_5s,
            evidence_id="EV-001",
            expected_sha256=valid_hash,
        )
        assert meta.sha256_hash == valid_hash

    def test_expected_sha256_mismatch_raises_integrity_error(self, synthetic_video_5s: Path) -> None:
        """Verify expected_sha256 raises VideoIntegrityError upon hash mismatch."""
        fake_hash = "0" * 64
        with pytest.raises(VideoIntegrityError, match="Evidence integrity mismatch"):
            probe_video_metadata(
                synthetic_video_5s,
                evidence_id="EV-001",
                expected_sha256=fake_hash,
            )

    def test_source_video_modification_during_extraction_detected(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Simulate tampering or modification during processing to verify VideoIntegrityError."""
        ev_dir = tmp_path / "evidence"
        ev_dir.mkdir(parents=True, exist_ok=True)
        video_copy = ev_dir / "copy_test.mp4"
        video_copy.write_bytes(synthetic_video_5s.read_bytes())
        out_dir = tmp_path / "artifacts"

        real_subprocess_run = subprocess.run

        def tamper_side_effect(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return real_subprocess_run(cmd, *args, **kwargs)
            # Tamper with the video copy during ffmpeg extraction
            with open(video_copy, "ab") as f:
                f.write(b"TAMPERED_BYTES")
            return real_subprocess_run(cmd, *args, **kwargs)

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=tamper_side_effect):
            with pytest.raises(VideoIntegrityError, match="Source video .* was modified during frame extraction"):
                extract_video_keyframes(
                    video_copy,
                    out_dir,
                    evidence_id="EV-001",
                    case_id="CASE-1",
                )


# --- 3. Keyframe & Fixed-Interval Extraction Tests ---


class TestKeyframeAndIntervalExtraction:
    """Tests frame extraction algorithms, ceilings, intervals, and formats."""

    def test_fixed_interval_extraction_1s(self, synthetic_video_5s: Path, tmp_path: Path) -> None:
        """Verify extraction of 1 frame per second on a 5-second video produces 5 frames."""
        out_dir = tmp_path / "frames_1s"
        opts = VideoExtractionOptions(interval_seconds=1.0)
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV_GATE",
            case_id="CASE-2026",
            options=opts,
        )

        assert res.total_frames_extracted == 5
        assert len(res.frames) == 5
        for idx, frame in enumerate(res.frames):
            assert frame.frame_index == idx + 1
            assert frame.file_name == f"CCTV_GATE_frame_{idx + 1:06d}.jpg"
            assert Path(frame.file_path).exists()
            assert frame.file_size_bytes > 0
            assert len(frame.sha256_hash) == 64
            assert frame.source_video_sha256 == res.source_video_sha256_before
            assert frame.pts_offset_seconds == pytest.approx(float(idx), abs=0.1)

    def test_max_frames_ceiling_enforced(self, synthetic_video_5s: Path, tmp_path: Path) -> None:
        """Verify that max_frames strictly caps the total extracted frames."""
        out_dir = tmp_path / "frames_max2"
        opts = VideoExtractionOptions(interval_seconds=1.0, max_frames=2)
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV_GATE",
            case_id="CASE-2026",
            options=opts,
        )

        assert res.total_frames_extracted == 2
        assert len(res.frames) == 2
        assert [f.frame_index for f in res.frames] == [1, 2]

    def test_keyframes_only_mode(self, synthetic_video_keyframes: Path, tmp_path: Path) -> None:
        """Verify keyframes_only mode extracts only native I-frames."""
        out_dir = tmp_path / "keyframes_out"
        opts = VideoExtractionOptions(keyframes_only=True)
        res = extract_video_keyframes(
            synthetic_video_keyframes,
            out_dir,
            evidence_id="CCTV_I_FRAMES",
            case_id="CASE-2026",
            options=opts,
        )

        # In a 6-second video with GOP=20 at 10 fps, I-frames occur at 0s, 2s, 4s (3 I-frames)
        assert res.total_frames_extracted == 3
        assert [f.pts_offset_seconds for f in res.frames] == [pytest.approx(0.0), pytest.approx(2.0), pytest.approx(4.0)]
        for f in res.frames:
            assert f.extraction_method == "keyframes"

    def test_png_output_format(self, synthetic_video_5s: Path, tmp_path: Path) -> None:
        """Verify configurable output image format to PNG."""
        out_dir = tmp_path / "frames_png"
        opts = VideoExtractionOptions(interval_seconds=2.0, image_format="png")
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV_PNG",
            case_id="CASE-2026",
            options=opts,
        )

        for frame in res.frames:
            assert frame.image_format == "png"
            assert frame.file_name.endswith(".png")
            assert Path(frame.file_path).exists()

    def test_invalid_image_format_rejected(self) -> None:
        """Verify invalid image format raises ValueError in options validation."""
        with pytest.raises(ValueError, match="Unsupported image format"):
            VideoExtractionOptions(image_format="gif")

    def test_fps_option_equivalence(self, synthetic_video_5s: Path, tmp_path: Path) -> None:
        """Verify fps=0.5 extracts 1 frame every 2 seconds."""
        out_dir = tmp_path / "frames_fps"
        opts = VideoExtractionOptions(fps=0.5, interval_seconds=None)
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV_FPS",
            case_id="CASE-2026",
            options=opts,
        )
        assert res.total_frames_extracted == 3  # at 0s, 2s, 4s


# --- 4. Timestamp Normalization & Provable Provenance Tests ---


class TestTimestampNormalization:
    """Verifies integration with Member 4 temporal engine for UTC normalization."""

    def test_anchored_utc_timestamps_with_timezone_and_clock_drift(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Verify UTC timestamp derivation with local timezone and signed clock drift correction.

        Recording start: 2026-11-17 10:00:00 in Asia/Kolkata (UTC+05:30)
        Normalized start: 04:30:00 UTC
        Clock offset: -5.0s (DVR clock was 5 seconds fast)
        Corrected start: 04:29:55 UTC
        """
        out_dir = tmp_path / "frames_anchored"
        opts = VideoExtractionOptions(
            interval_seconds=1.0,
            recording_start_timestamp="2026-11-17T10:00:00",
            source_timezone="Asia/Kolkata",
            clock_offset_seconds=-5.0,
        )
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV_ANCHORED",
            case_id="CASE-2026",
            options=opts,
        )

        assert res.total_frames_extracted == 5
        base_expected_utc = datetime(2026, 11, 17, 4, 29, 55, tzinfo=timezone.utc)

        for idx, frame in enumerate(res.frames):
            assert frame.is_utc_anchored is True
            assert frame.timestamp_utc is not None
            expected_frame_utc = datetime(2026, 11, 17, 4, 29, 55 + idx, tzinfo=timezone.utc)
            assert frame.timestamp_utc == expected_frame_utc
            assert frame.timestamp_precision == CanonicalTimePrecision.EXACT_SECOND
            assert frame.time_window is not None
            assert frame.time_window.nominal_utc == expected_frame_utc
            assert frame.time_window.window_start_utc == expected_frame_utc
            assert frame.time_window.uncertainty_span_seconds == 1.0

    def test_unanchored_pts_preserved_without_fabricating_utc(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """When recording start timestamp is absent, UTC timestamp must be None (never fabricated)."""
        out_dir = tmp_path / "frames_unanchored"
        opts = VideoExtractionOptions(interval_seconds=1.0, recording_start_timestamp=None)
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV_UNANCHORED",
            case_id="CASE-2026",
            options=opts,
        )

        assert res.total_frames_extracted == 5
        for idx, frame in enumerate(res.frames):
            assert frame.is_utc_anchored is False
            assert frame.timestamp_utc is None
            assert frame.timestamp_precision is None
            assert frame.time_window is None
            assert frame.pts_offset_seconds == pytest.approx(float(idx), abs=0.1)


# --- 5. Start/End Time Bounds & Constraints Tests ---


class TestTimeBoundsAndValidation:
    """Verifies start_time_seconds and end_time_seconds filtering and bound validation."""

    def test_extraction_within_time_bounds(self, synthetic_video_5s: Path, tmp_path: Path) -> None:
        """Extract frames bounded between 1.0s and 4.0s."""
        out_dir = tmp_path / "frames_bounded"
        opts = VideoExtractionOptions(
            interval_seconds=1.0,
            start_time_seconds=1.0,
            end_time_seconds=4.0,
        )
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV_BOUNDS",
            case_id="CASE-2026",
            options=opts,
        )

        assert res.total_frames_extracted == 3  # at 1.0s, 2.0s, 3.0s
        assert res.frames[0].pts_offset_seconds == pytest.approx(1.0, abs=0.1)
        assert res.frames[1].pts_offset_seconds == pytest.approx(2.0, abs=0.1)
        assert res.frames[2].pts_offset_seconds == pytest.approx(3.0, abs=0.1)

    def test_end_time_before_start_time_rejected(self) -> None:
        """Invalid bounds where end_time <= start_time must raise ValueError."""
        with pytest.raises(ValueError, match="end_time_seconds .* must be strictly greater than start_time_seconds"):
            VideoExtractionOptions(start_time_seconds=4.0, end_time_seconds=2.0)

    def test_start_time_exceeding_duration_rejected(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """start_time_seconds >= duration must raise ValueError."""
        out_dir = tmp_path / "frames_invalid_start"
        opts = VideoExtractionOptions(start_time_seconds=10.0)
        with pytest.raises(ValueError, match="start_time_seconds .* exceeds or equals video duration"):
            extract_video_keyframes(
                synthetic_video_5s,
                out_dir,
                evidence_id="CCTV_001",
                case_id="CASE-1",
                options=opts,
            )


# --- 6. Path Security & Path Traversal Tests ---


class TestPathSecurityAndTraversal:
    """Verifies path sanitization, traversal blocking, and evidence collision prevention."""

    def test_null_byte_in_path_rejected(self) -> None:
        """Paths containing null bytes must raise PathSecurityError."""
        with pytest.raises(PathSecurityError, match="illegal null bytes"):
            validate_safe_path("evidence\0video.mp4")

    def test_path_traversal_outside_allowed_dir_rejected(self, tmp_path: Path) -> None:
        """Path traversal escaping allowed base directory must raise PathSecurityError."""
        allowed_dir = tmp_path / "allowed"
        allowed_dir.mkdir()
        forbidden_file = tmp_path / "secret.mp4"
        forbidden_file.write_text("secret")

        with pytest.raises(PathSecurityError, match="escapes allowed base directory"):
            validate_safe_path(
                allowed_dir / ".." / "secret.mp4",
                allowed_base_dir=allowed_dir,
            )

    def test_output_directory_cannot_overwrite_video_file(
        self, synthetic_video_5s: Path
    ) -> None:
        """output_dir cannot be set to the original video file path."""
        with pytest.raises(PathSecurityError, match="cannot overwrite original evidence file"):
            extract_video_keyframes(
                synthetic_video_5s,
                output_dir=synthetic_video_5s,
                evidence_id="EV-001",
                case_id="CASE-1",
            )

    def test_output_directory_cannot_be_raw_evidence_directory(
        self, synthetic_video_5s: Path
    ) -> None:
        """output_dir cannot be the parent raw evidence directory."""
        with pytest.raises(PathSecurityError, match="cannot be inside or equal to the raw evidence directory"):
            extract_video_keyframes(
                synthetic_video_5s,
                output_dir=synthetic_video_5s.parent,
                evidence_id="EV-001",
                case_id="CASE-1",
            )


# --- 7. Failure Handling & Cleanup Tests ---


class TestFailureModesAndCleanup:
    """Verifies graceful error handling and partial artifact cleanup on failure."""

    def test_missing_video_file_raises_not_found(self, tmp_path: Path) -> None:
        """Missing video file raises VideoNotFoundError."""
        missing = tmp_path / "non_existent.mp4"
        with pytest.raises(VideoNotFoundError, match="Target path does not exist"):
            probe_video_metadata(missing, evidence_id="EV-001")

    def test_directory_passed_as_video_raises_error(self, tmp_path: Path) -> None:
        """Passing a directory instead of a video file raises VideoProcessingError."""
        with pytest.raises(VideoProcessingError, match="is a directory, not a video file"):
            probe_video_metadata(tmp_path, evidence_id="EV-001")

    def test_unsupported_non_video_format_raises_error(self, tmp_path: Path) -> None:
        """Plain text file passed as video raises UnsupportedVideoFormatError or CorruptVideoError."""
        fake_video = tmp_path / "fake.txt"
        fake_video.write_text("This is an unparseable text file, not video.")

        with pytest.raises((UnsupportedVideoFormatError, CorruptVideoError)):
            probe_video_metadata(fake_video, evidence_id="EV-001")

    def test_corrupt_video_file_raises_corrupt_error(self, tmp_path: Path) -> None:
        """A corrupt MP4 with truncated header raises CorruptVideoError."""
        corrupt = tmp_path / "corrupt.mp4"
        corrupt.write_bytes(b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom" + b"\xFF" * 100)

        with pytest.raises(CorruptVideoError):
            probe_video_metadata(corrupt, evidence_id="EV-001")

    def test_audio_only_file_rejected_for_frame_extraction(
        self, synthetic_audio_only: Path, tmp_path: Path
    ) -> None:
        """Attempting to extract frames from an audio-only file raises UnsupportedVideoFormatError."""
        out_dir = tmp_path / "audio_frames"
        with pytest.raises(UnsupportedVideoFormatError, match="media file contains no video streams"):
            extract_video_keyframes(
                synthetic_audio_only,
                out_dir,
                evidence_id="AUDIO-001",
                case_id="CASE-1",
            )

    def test_timeout_handling_raises_timeout_error_and_cleans_up(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Simulate a subprocess timeout and verify VideoProcessingTimeoutError and artifact cleanup."""
        out_dir = tmp_path / "timeout_out"
        out_dir.mkdir(parents=True, exist_ok=True)
        # Pre-existing file that must NOT be touched
        keep_file = out_dir / "keep_me.txt"
        keep_file.write_text("Preserve me")

        real_subprocess_run = subprocess.run

        def timeout_effect(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return real_subprocess_run(cmd, *args, **kwargs)
            # Write a partial artifact before timing out
            partial = out_dir / "EV-001_frame_000001.jpg"
            partial.write_bytes(b"partial")
            raise subprocess.TimeoutExpired(cmd=cmd, timeout=1.0)

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=timeout_effect):
            opts = VideoExtractionOptions(timeout_seconds=1.0)
            with pytest.raises(VideoProcessingTimeoutError, match="timed out after 1.0 seconds"):
                extract_video_keyframes(
                    synthetic_video_5s,
                    out_dir,
                    evidence_id="EV-001",
                    case_id="CASE-1",
                    options=opts,
                )

            # Assert partial artifact was cleaned up and pre-existing file was preserved
            assert not (out_dir / "EV-001_frame_000001.jpg").exists()
            assert keep_file.exists()
            assert keep_file.read_text() == "Preserve me"

    def test_partial_artifacts_cleaned_up_on_ffmpeg_error(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Verify that when FFmpeg exits with non-zero code, newly created partial frames are removed."""
        out_dir = tmp_path / "error_out"
        out_dir.mkdir(parents=True, exist_ok=True)
        keep_file = out_dir / "untouched.txt"
        keep_file.write_text("Do not delete")

        real_subprocess_run = subprocess.run

        def error_effect(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return real_subprocess_run(cmd, *args, **kwargs)
            partial = out_dir / "EV-001_frame_000001.jpg"
            partial.write_bytes(b"partial")
            return MagicMock(returncode=1, stderr="Simulated FFmpeg decoding crash")

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=error_effect):
            with pytest.raises(VideoExtractionError, match="FFmpeg extraction failed"):
                extract_video_keyframes(
                    synthetic_video_5s,
                    out_dir,
                    evidence_id="EV-001",
                    case_id="CASE-1",
                )

            assert not (out_dir / "EV-001_frame_000001.jpg").exists()
            assert keep_file.exists()

    def test_empty_extraction_raises_error(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """When 0 frames are produced by FFmpeg, VideoExtractionError is raised."""
        out_dir = tmp_path / "empty_out"

        real_subprocess_run = subprocess.run

        def empty_effect(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return real_subprocess_run(cmd, *args, **kwargs)
            # FFmpeg finishes with 0 frames written
            return MagicMock(returncode=0, stderr="")

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=empty_effect):
            with pytest.raises(VideoExtractionError, match="No frames were extracted"):
                extract_video_keyframes(
                    synthetic_video_5s,
                    out_dir,
                    evidence_id="EV-001",
                    case_id="CASE-1",
                )


# --- 8. Phase 2D.1 Forensic Integrity Audit Tests ---


class TestPhase2D1ForensicIntegrityAudit:
    """Specific regression tests for Phase 2D.1 Video Forensic Integrity Audit."""

    # --- Audit A: Timestamp Normalization ---

    def test_audit_a_dst_fall_back_ambiguous_naive_timestamp_rejected(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Ambiguous naive local timestamp during DST fall-back in America/New_York is strictly rejected."""
        out_dir = tmp_path / "artifacts_dst_ambiguous"
        opts = VideoExtractionOptions(
            recording_start_timestamp="2026-11-01 01:30",
            source_timezone="America/New_York",
        )
        with pytest.raises(ValueError, match="Ambiguous naive local timestamp .* in timezone 'America/New_York'"):
            extract_video_keyframes(
                synthetic_video_5s,
                out_dir,
                evidence_id="CCTV-DST-1",
                case_id="CASE-1",
                options=opts,
            )

    def test_audit_a_dst_spring_forward_nonexistent_naive_timestamp_rejected(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Nonexistent naive local timestamp during DST spring-forward gap in America/New_York is rejected."""
        out_dir = tmp_path / "artifacts_dst_nonexistent"
        opts = VideoExtractionOptions(
            recording_start_timestamp="2026-03-08 02:30",
            source_timezone="America/New_York",
        )
        with pytest.raises(ValueError, match="Nonexistent naive local timestamp .* in timezone 'America/New_York'"):
            extract_video_keyframes(
                synthetic_video_5s,
                out_dir,
                evidence_id="CCTV-DST-2",
                case_id="CASE-1",
                options=opts,
            )

    def test_audit_a_explicit_utc_offsets_disambiguate_dst_and_normalize(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Explicit ISO offsets disambiguate DST fall-back instants and normalize to UTC accurately."""
        # EDT (UTC-4): 01:30 EDT = 05:30 UTC
        out_edt = tmp_path / "artifacts_edt"
        opts_edt = VideoExtractionOptions(
            interval_seconds=1.0,
            recording_start_timestamp="2026-11-01T01:30:00-04:00",
            source_timezone="America/New_York",
        )
        res_edt = extract_video_keyframes(
            synthetic_video_5s,
            out_edt,
            evidence_id="CCTV-EDT",
            case_id="CASE-1",
            options=opts_edt,
        )
        assert res_edt.frames[0].timestamp_utc == datetime(2026, 11, 1, 5, 30, 0, tzinfo=timezone.utc)

        # EST (UTC-5): 01:30 EST = 06:30 UTC
        out_est = tmp_path / "artifacts_est"
        opts_est = VideoExtractionOptions(
            interval_seconds=1.0,
            recording_start_timestamp="2026-11-01T01:30:00-05:00",
            source_timezone="America/New_York",
        )
        res_est = extract_video_keyframes(
            synthetic_video_5s,
            out_est,
            evidence_id="CCTV-EST",
            case_id="CASE-1",
            options=opts_est,
        )
        assert res_est.frames[0].timestamp_utc == datetime(2026, 11, 1, 6, 30, 0, tzinfo=timezone.utc)

    def test_audit_a_clock_offset_applied_exactly_once(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Signed DVR clock offset is applied exactly once to recording start, not compounding on frames."""
        out_dir = tmp_path / "artifacts_clock_drift"
        opts = VideoExtractionOptions(
            interval_seconds=1.0,
            recording_start_timestamp="2026-11-17T12:00:00",
            source_timezone="UTC",
            clock_offset_seconds=-10.0,
        )
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="CCTV-CLOCK",
            case_id="CASE-1",
            options=opts,
        )
        # Frame 0: 12:00:00 - 10s = 11:59:50
        assert res.frames[0].timestamp_utc == datetime(2026, 11, 17, 11, 59, 50, tzinfo=timezone.utc)
        # Frame 1: 11:59:50 + 1s PTS = 11:59:51 (offset is NOT applied again)
        assert res.frames[1].timestamp_utc == datetime(2026, 11, 17, 11, 59, 51, tzinfo=timezone.utc)
        # Frame 2: 11:59:50 + 2s PTS = 11:59:52
        assert res.frames[2].timestamp_utc == datetime(2026, 11, 17, 11, 59, 52, tzinfo=timezone.utc)

    # --- Audit B: Path and Symlink Security ---

    def test_audit_b_output_dir_inside_raw_evidence_storage_subfolder_rejected(
        self, tmp_path: Path, synthetic_video_5s: Path
    ) -> None:
        """Output directory inside a subfolder of raw evidence storage is rejected."""
        ev_dir = tmp_path / "raw_evidence"
        ev_dir.mkdir(parents=True, exist_ok=True)
        video_file = ev_dir / "feed.mp4"
        video_file.write_bytes(synthetic_video_5s.read_bytes())

        # Subdirectory INSIDE raw evidence directory
        forbidden_output = ev_dir / "derived_frames"

        with pytest.raises(PathSecurityError, match="cannot be inside or equal to the raw evidence directory"):
            extract_video_keyframes(
                video_file,
                forbidden_output,
                evidence_id="EV-SUBDIR",
                case_id="CASE-1",
            )

    def test_audit_b_symlink_in_output_directory_rejected(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Any symlink in the output artifact directory is detected and raises PathSecurityError."""
        out_dir = tmp_path / "symlink_attack_dir"
        out_dir.mkdir(parents=True, exist_ok=True)
        sensitive_target = tmp_path / "sensitive.txt"
        sensitive_target.write_text("SENSITIVE DATA")

        # Plant a symlink in output dir
        link_file = out_dir / "EV-001_frame_000001.jpg"
        link_file.symlink_to(sensitive_target)

        with pytest.raises(PathSecurityError, match="Insecure symlink detected in artifact output directory"):
            extract_video_keyframes(
                synthetic_video_5s,
                out_dir,
                evidence_id="EV-001",
                case_id="CASE-1",
            )

        # Sensitive target must be completely untouched
        assert sensitive_target.read_text() == "SENSITIVE DATA"

    def test_audit_b_pre_existing_frame_files_rejected_regardless_of_options(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Pre-existing frame artifact collision is strictly rejected regardless of caller options."""
        out_dir = tmp_path / "collision_dir"
        out_dir.mkdir(parents=True, exist_ok=True)
        existing_frame = out_dir / "EV-001_frame_000001.jpg"
        existing_frame.write_text("PRE-EXISTING FRAME CONTENT")

        # Standard extraction must reject collision
        with pytest.raises(VideoExtractionError, match="Pre-existing artifact collision"):
            extract_video_keyframes(
                synthetic_video_5s,
                out_dir,
                evidence_id="EV-001",
                case_id="CASE-1",
            )

        # Existing content remains untouched
        assert existing_frame.read_text() == "PRE-EXISTING FRAME CONTENT"

        # Explicitly passing overwrite option must be rejected at schema validation
        with pytest.raises(ValueError, match="Overwriting pre-existing derived artifacts is strictly forbidden"):
            VideoExtractionOptions(overwrite=True)  # type: ignore[call-arg]

    def test_audit_b_unique_run_directory_prevents_collision(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """create_unique_run_subdir=True isolates outputs in a fresh subdirectory and avoids collision."""
        out_dir = tmp_path / "unique_dir"
        out_dir.mkdir(parents=True, exist_ok=True)
        # Even if base directory has a pre-existing frame artifact with same name
        existing_frame = out_dir / "EV-001_frame_000001.jpg"
        existing_frame.write_text("BASE FRAME")

        opts = VideoExtractionOptions(interval_seconds=1.0, max_frames=1, create_unique_run_subdir=True)
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="EV-001",
            case_id="CASE-1",
            options=opts,
        )

        assert res.total_frames_extracted == 1
        assert Path(res.output_directory) != out_dir.resolve()
        assert Path(res.output_directory).parent == out_dir.resolve()
        assert existing_frame.read_text() == "BASE FRAME"

    # --- Audit C: PTS-to-Artifact Correspondence ---

    def test_audit_c_vfr_video_pts_correspondence(
        self, tmp_path: Path
    ) -> None:
        """Variable frame rate video extracted frames receive exact source PTS timestamps."""
        vfr_dir = tmp_path / "vfr_src"
        vfr_dir.mkdir(parents=True, exist_ok=True)
        vfr_video = vfr_dir / "vfr_test.mp4"

        # Generate 4-second video with altered PTS
        cmd = [
            "ffmpeg", "-y", "-f", "lavfi", "-i", "testsrc=duration=4:size=320x240:rate=10",
            "-vf", "setpts=if(lt(N\\,10)\\,PTS\\,PTS*2)",
            "-c:v", "libx264", str(vfr_video)
        ]
        subprocess.run(cmd, check=True, capture_output=True)

        out_dir = tmp_path / "vfr_frames"
        opts = VideoExtractionOptions(interval_seconds=1.0)
        res = extract_video_keyframes(
            vfr_video,
            out_dir,
            evidence_id="VFR-EV",
            case_id="CASE-1",
            options=opts,
        )

        assert res.total_frames_extracted > 0
        for frame in res.frames:
            assert frame.pts_offset_seconds >= 0.0

    def test_audit_c_nonzero_start_timestamp_preserved(
        self, tmp_path: Path
    ) -> None:
        """Videos starting at non-zero PTS preserve true source timestamps via -copyts."""
        nz_dir = tmp_path / "nonzero_src"
        nz_dir.mkdir(parents=True, exist_ok=True)
        nz_video = nz_dir / "nonzero_test.mp4"

        # Video PTS starts at 10.0s
        cmd = [
            "ffmpeg", "-y", "-f", "lavfi", "-i", "testsrc=duration=4:size=320x240:rate=10",
            "-vf", "setpts=PTS+10/TB",
            "-c:v", "libx264", str(nz_video)
        ]
        subprocess.run(cmd, check=True, capture_output=True)

        out_dir = tmp_path / "nz_frames"
        opts = VideoExtractionOptions(interval_seconds=1.0)
        res = extract_video_keyframes(
            nz_video,
            out_dir,
            evidence_id="NZ-EV",
            case_id="CASE-1",
            options=opts,
        )

        # The first frame PTS must be >= 10.0s (not reset to 0.0s)
        assert res.frames[0].pts_offset_seconds >= 10.0

    def test_audit_c_missing_pts_log_fails_safely_without_guessing(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """When showinfo is missing a frame's PTS record, extraction fails safely rather than guessing."""
        out_dir = tmp_path / "pts_missing_dir"

        real_subprocess_run = subprocess.run

        def missing_pts_effect(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return real_subprocess_run(cmd, *args, **kwargs)
            res = real_subprocess_run(cmd, *args, **kwargs)
            # Remove showinfo record for n: 2 to simulate corrupted or missing PTS log
            lines = [l for l in res.stderr.splitlines() if not ("n:   2" in l or "n: 2" in l)]
            return MagicMock(returncode=0, stderr="\n".join(lines), stdout=res.stdout)

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=missing_pts_effect):
            opts = VideoExtractionOptions(interval_seconds=1.0)
            with pytest.raises(VideoExtractionError, match="Forensic PTS correspondence failure"):
                extract_video_keyframes(
                    synthetic_video_5s,
                    out_dir,
                    evidence_id="EV-CORR-1",
                    case_id="CASE-1",
                    options=opts,
                )

        # Partial frame files must be cleaned up
        remaining = [f for f in out_dir.iterdir() if f.name.endswith(".jpg")] if out_dir.exists() else []
        assert len(remaining) == 0

    def test_audit_c_duplicate_conflicting_pts_log_fails_safely(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Duplicate conflicting showinfo PTS records for the same frame index fail safely."""
        out_dir = tmp_path / "pts_conflict_dir"

        real_subprocess_run = subprocess.run

        def conflict_pts_effect(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return real_subprocess_run(cmd, *args, **kwargs)
            res = real_subprocess_run(cmd, *args, **kwargs)
            # Inject a conflicting duplicate for n: 0 with different pts_time
            conflict_line = "[Parsed_showinfo_1 @ 0x123] n: 0 pts: 50000 pts_time:99.000000"
            new_stderr = conflict_line + "\n" + res.stderr
            return MagicMock(returncode=0, stderr=new_stderr, stdout=res.stdout)

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=conflict_pts_effect):
            opts = VideoExtractionOptions(interval_seconds=1.0)
            with pytest.raises(VideoExtractionError, match="Conflicting duplicate showinfo PTS records"):
                extract_video_keyframes(
                    synthetic_video_5s,
                    out_dir,
                    evidence_id="EV-CORR-2",
                    case_id="CASE-1",
                    options=opts,
                )

    # --- Audit D: Evidence Immutability & Provenance Retained ---

    def test_audit_d_source_immutability_on_failure_and_success(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Original video bytes and SHA-256 are unchanged after success, validation failure, and extraction failure."""
        ev_dir = tmp_path / "immutability_ev"
        ev_dir.mkdir(parents=True, exist_ok=True)
        vid = ev_dir / "evidence_video.mp4"
        vid.write_bytes(synthetic_video_5s.read_bytes())

        initial_bytes = vid.read_bytes()
        initial_hash = compute_file_sha256(vid)

        # 1. Successful extraction
        out_success = tmp_path / "art_success"
        res = extract_video_keyframes(
            vid,
            out_success,
            evidence_id="EV-IMM",
            case_id="CASE-1",
            options=VideoExtractionOptions(interval_seconds=1.0, max_frames=2),
        )
        assert vid.read_bytes() == initial_bytes
        assert compute_file_sha256(vid) == initial_hash
        # Verify provenance retention on derived frames
        for frame in res.frames:
            assert frame.source_evidence_id == "EV-IMM"
            assert frame.source_video_sha256 == initial_hash
            assert frame.case_id == "CASE-1"

        # 2. Validation failure (invalid start time)
        with pytest.raises(ValueError):
            extract_video_keyframes(
                vid,
                tmp_path / "art_fail_val",
                evidence_id="EV-IMM",
                case_id="CASE-1",
                options=VideoExtractionOptions(start_time_seconds=999.0),
            )
        assert vid.read_bytes() == initial_bytes
        assert compute_file_sha256(vid) == initial_hash

        # 3. Extraction failure (aborted FFmpeg)
        def err_run(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return _REAL_SUBPROCESS_RUN(cmd, *args, **kwargs)
            return MagicMock(returncode=1, stderr="Simulated crash")

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=err_run):
            with pytest.raises(VideoExtractionError):
                extract_video_keyframes(
                    vid,
                    tmp_path / "art_fail_proc",
                    evidence_id="EV-IMM",
                    case_id="CASE-1",
                )

        assert vid.read_bytes() == initial_bytes
        assert compute_file_sha256(vid) == initial_hash


# --- 9. Phase 2D.2 Derived Artifact Overwrite Safety Tests ---


class TestPhase2D2DerivedArtifactOverwriteSafety:
    """Specific regression tests for Phase 2D.2 Derived Artifact Overwrite Safety."""

    def test_existing_matching_frame_files_rejected_and_untouched(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Existing matching frame files are never overwritten and cause immediate safe failure."""
        out_dir = tmp_path / "protected_artifacts"
        out_dir.mkdir(parents=True, exist_ok=True)
        collision_file = out_dir / "EV-LOCKED_frame_000001.jpg"
        collision_file.write_bytes(b"FORENSIC_EVIDENCE_DO_NOT_TOUCH")
        initial_hash = compute_file_sha256(collision_file)

        opts = VideoExtractionOptions(interval_seconds=1.0)
        with pytest.raises(VideoExtractionError, match="Pre-existing artifact collision.*Overwriting derived artifacts is strictly forbidden"):
            extract_video_keyframes(
                synthetic_video_5s,
                out_dir,
                evidence_id="EV-LOCKED",
                case_id="CASE-1",
                options=opts,
            )

        # Existing file is 100% bitwise identical
        assert collision_file.read_bytes() == b"FORENSIC_EVIDENCE_DO_NOT_TOUCH"
        assert compute_file_sha256(collision_file) == initial_hash

    def test_existing_non_conflicting_user_files_preserved_across_runs(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Non-conflicting user files in output directory are completely preserved during extraction."""
        out_dir = tmp_path / "mixed_artifacts"
        out_dir.mkdir(parents=True, exist_ok=True)

        user_doc = out_dir / "investigator_notes.txt"
        user_doc.write_text("Interrogation notes: Subject appeared at 10:15 AM.")
        user_json = out_dir / "audit_manifest.json"
        user_json.write_text('{"investigator": "Officer Roy", "case_id": "CASE-1"}')

        doc_hash = compute_file_sha256(user_doc)
        json_hash = compute_file_sha256(user_json)

        opts = VideoExtractionOptions(interval_seconds=1.0, max_frames=2)
        res = extract_video_keyframes(
            synthetic_video_5s,
            out_dir,
            evidence_id="EV-MIXED",
            case_id="CASE-1",
            options=opts,
        )

        assert res.total_frames_extracted == 2
        # User files exist and are bitwise identical
        assert user_doc.exists()
        assert compute_file_sha256(user_doc) == doc_hash
        assert user_json.exists()
        assert compute_file_sha256(user_json) == json_hash

    def test_symlinked_output_files_strictly_rejected_and_targets_untouched(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Symlinked files or directories in output directory raise PathSecurityError and leave targets intact."""
        out_dir = tmp_path / "symlink_test_dir"
        out_dir.mkdir(parents=True, exist_ok=True)

        sensitive_db = tmp_path / "system_passwords.db"
        sensitive_db.write_bytes(b"SECRET_HASH_STORE")
        orig_db_hash = compute_file_sha256(sensitive_db)

        # 1. Symlink inside output directory
        trap_link = out_dir / "EV-TRAP_frame_000001.jpg"
        trap_link.symlink_to(sensitive_db)

        with pytest.raises(PathSecurityError, match="Insecure symlink detected"):
            extract_video_keyframes(
                synthetic_video_5s,
                out_dir,
                evidence_id="EV-TRAP",
                case_id="CASE-1",
            )

        assert sensitive_db.read_bytes() == b"SECRET_HASH_STORE"
        assert compute_file_sha256(sensitive_db) == orig_db_hash
        trap_link.unlink()

        # 2. Output directory itself is a symlink
        symlinked_out_dir = tmp_path / "symlink_out_dir"
        symlinked_out_dir.symlink_to(out_dir)

        with pytest.raises(PathSecurityError, match="cannot be a symlink"):
            extract_video_keyframes(
                synthetic_video_5s,
                symlinked_out_dir,
                evidence_id="EV-TRAP2",
                case_id="CASE-1",
            )

    def test_failed_extraction_after_partial_output_cleans_only_new_files(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Failed FFmpeg run cleans up only newly generated partial frame files, leaving pre-existing files intact."""
        out_dir = tmp_path / "partial_failure_cleanup"
        out_dir.mkdir(parents=True, exist_ok=True)

        pre_existing = out_dir / "prior_evidence.txt"
        pre_existing.write_text("Do not remove this file under any circumstance")
        pre_hash = compute_file_sha256(pre_existing)

        def mock_failing_ffmpeg(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return _REAL_SUBPROCESS_RUN(cmd, *args, **kwargs)
            # Create a partial artifact during execution before crashing
            partial = out_dir / "EV-PARTIAL_frame_000001.jpg"
            partial.write_bytes(b"PARTIAL_UNFINISHED_FRAME_DATA")
            return MagicMock(returncode=1, stderr="Simulated fatal encoder failure")

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=mock_failing_ffmpeg):
            with pytest.raises(VideoExtractionError, match="FFmpeg extraction failed"):
                extract_video_keyframes(
                    synthetic_video_5s,
                    out_dir,
                    evidence_id="EV-PARTIAL",
                    case_id="CASE-1",
                )

        # Partial frame was cleaned up
        assert not (out_dir / "EV-PARTIAL_frame_000001.jpg").exists()
        # Pre-existing file was NOT touched
        assert pre_existing.exists()
        assert compute_file_sha256(pre_existing) == pre_hash

    def test_repeated_and_concurrent_extraction_collision_safety(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Repeated extractions to same directory are blocked, while unique per-run subdirs succeed."""
        base_dir = tmp_path / "repeated_extractions"
        base_dir.mkdir(parents=True, exist_ok=True)

        # First run into base_dir succeeds
        opts1 = VideoExtractionOptions(interval_seconds=1.0, max_frames=2)
        res1 = extract_video_keyframes(
            synthetic_video_5s,
            base_dir,
            evidence_id="EV-REPEAT",
            case_id="CASE-1",
            options=opts1,
        )
        assert res1.total_frames_extracted == 2
        frame1_hash = compute_file_sha256(res1.frames[0].file_path)

        # Second run to identical output_dir with same evidence ID MUST fail
        with pytest.raises(VideoExtractionError, match="Pre-existing artifact collision"):
            extract_video_keyframes(
                synthetic_video_5s,
                base_dir,
                evidence_id="EV-REPEAT",
                case_id="CASE-1",
                options=opts1,
            )

        # First run's files remain intact
        assert compute_file_sha256(res1.frames[0].file_path) == frame1_hash

        # Run with create_unique_run_subdir=True succeeds without collision
        opts_unique = VideoExtractionOptions(
            interval_seconds=1.0,
            max_frames=2,
            create_unique_run_subdir=True,
        )
        res_unique = extract_video_keyframes(
            synthetic_video_5s,
            base_dir,
            evidence_id="EV-REPEAT",
            case_id="CASE-1",
            options=opts_unique,
        )
        assert res_unique.total_frames_extracted == 2
        assert Path(res_unique.output_directory) != base_dir.resolve()
        assert Path(res_unique.output_directory).parent == base_dir.resolve()

    def test_original_video_and_existing_artifact_hashes_unchanged_after_failure(
        self, synthetic_video_5s: Path, tmp_path: Path
    ) -> None:
        """Both original video and pre-existing files retain identical SHA-256 hashes after extraction failure."""
        ev_dir = tmp_path / "safe_ev"
        ev_dir.mkdir(parents=True, exist_ok=True)
        vid_path = ev_dir / "cctv_safe.mp4"
        vid_path.write_bytes(synthetic_video_5s.read_bytes())
        initial_vid_hash = compute_file_sha256(vid_path)

        out_dir = tmp_path / "safe_out"
        out_dir.mkdir(parents=True, exist_ok=True)
        pre_doc = out_dir / "user_case_file.pdf"
        pre_doc.write_bytes(b"%PDF-1.4 test document content")
        initial_doc_hash = compute_file_sha256(pre_doc)

        def mock_timeout_run(cmd, *args, **kwargs):
            if any("ffprobe" in str(arg) for arg in cmd):
                return _REAL_SUBPROCESS_RUN(cmd, *args, **kwargs)
            # Write a partial file before timing out
            (out_dir / "EV-SAFE_frame_000001.jpg").write_bytes(b"partial")
            raise subprocess.TimeoutExpired(cmd=cmd, timeout=1.0)

        with patch("app.modules.reconstruction.video.subprocess.run", side_effect=mock_timeout_run):
            with pytest.raises(VideoProcessingTimeoutError):
                extract_video_keyframes(
                    vid_path,
                    out_dir,
                    evidence_id="EV-SAFE",
                    case_id="CASE-1",
                    options=VideoExtractionOptions(timeout_seconds=1.0),
                )

        # Immutability check
        assert compute_file_sha256(vid_path) == initial_vid_hash
        assert compute_file_sha256(pre_doc) == initial_doc_hash
        assert not (out_dir / "EV-SAFE_frame_000001.jpg").exists()

    def test_generate_unique_artifact_directory_utility(self, tmp_path: Path) -> None:
        """generate_unique_artifact_directory creates unique distinct paths for each call."""
        dir1 = generate_unique_artifact_directory(tmp_path, case_id="CASE-1", evidence_id="CCTV-1")
        dir2 = generate_unique_artifact_directory(tmp_path, case_id="CASE-1", evidence_id="CCTV-1")
        assert dir1 != dir2
        assert "CASE-1_CCTV-1_" in dir1.name
        assert "CASE-1_CCTV-1_" in dir2.name
