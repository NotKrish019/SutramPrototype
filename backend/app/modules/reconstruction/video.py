"""Forensic CCTV & Video Evidence Processing Module (Member 4 — Forensic Intelligence & System Integration).

Provides secure, read-only extraction of technical video metadata and deterministic
keyframe / interval frame extraction using FFprobe and FFmpeg subprocess CLI.

Key Forensic Guarantees:
1. Immutability of Original Evidence: Original video files are opened in strict read-only
   binary mode for SHA-256 computation and are never modified, overwritten, or re-encoded.
2. Cryptographic Provenance: Source video SHA-256 is verified both before and after processing.
   Every derived frame artifact is cryptographically bound to the source evidence SHA-256.
3. Separation of Artifacts: Derived frames are strictly stored in a distinct derived-artifact
   directory and never mixed with raw evidence storage.
4. Timestamp Normalization: Anchored UTC timestamps are derived via the Member 4 temporal
   engine (with timezone localization and clock drift correction) only when camera recording
   start metadata is provided; relative presentation timestamps (PTS) are never fabricated
   into false UTC timestamps.
5. Resource & Path Security: Subprocess calls use explicit argument lists (no shell interpolation),
   configurable timeouts, strict frame-count ceilings, and path-traversal prevention.

Scope: this module performs frame extraction with provenance only. It does not perform semantic
event recognition (no detection, classification or identification of frame contents), and it
never derives events from frames. Semantic analysis belongs to Member 1.
"""

from datetime import datetime, timedelta, timezone
import hashlib
import json
import logging
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
from typing import Any, Optional, Sequence
import uuid

from app.modules.reconstruction.temporal import (
    compute_uncertainty_window,
    parse_and_normalize_timestamp,
)
from app.schemas.reconstruction import (
    CanonicalTimePrecision,
    ExtractedFrameArtifact,
    NormalizedTimeWindow,
    VideoExtractionOptions,
    VideoExtractionResult,
    VideoMetadata,
    VideoStreamMetadata,
)

logger = logging.getLogger(__name__)

# Cached FFmpeg and FFprobe binary metadata
_CACHED_TOOL_METADATA: Optional[dict[str, str]] = None


# --- Custom Forensic Video Exceptions ---


class VideoProcessingError(Exception):
    """Base exception for forensic video processing errors."""

    pass


class VideoNotFoundError(VideoProcessingError, FileNotFoundError):
    """Raised when the specified video evidence file does not exist."""

    pass


class VideoIntegrityError(VideoProcessingError):
    """Raised when evidence integrity verification fails (e.g. SHA-256 mismatch or file modification)."""

    pass


class UnsupportedVideoFormatError(VideoProcessingError):
    """Raised when the input file is not a supported media container or lacks video streams."""

    pass


class CorruptVideoError(VideoProcessingError):
    """Raised when the video file is corrupt or unreadable by FFprobe/FFmpeg."""

    pass


class VideoExtractionError(VideoProcessingError):
    """Raised when FFmpeg frame extraction fails, produces empty results, or encounters errors."""

    pass


class VideoProcessingTimeoutError(VideoExtractionError):
    """Raised when an FFmpeg or FFprobe operation exceeds its configured timeout limit."""

    pass


class PathSecurityError(VideoProcessingError):
    """Raised when a path violates security constraints or attempts directory traversal."""

    pass


# --- Utility & Security Functions ---


def compute_file_sha256(file_path: Path | str, chunk_size: int = 65536) -> str:
    """Calculate the cryptographic SHA-256 hash of a file using streaming binary chunks.

    Never loads the entire file into memory, preserving system stability on multi-gigabyte footage.
    """
    path = Path(file_path)
    if not path.exists():
        raise VideoNotFoundError(f"Evidence file not found: '{file_path}'")
    if not path.is_file():
        raise VideoProcessingError(f"Expected a file for SHA-256 calculation, got directory: '{file_path}'")

    hasher = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(chunk_size):
            hasher.update(chunk)
    return hasher.hexdigest().lower()


def validate_safe_path(
    path: Path | str,
    *,
    allow_missing: bool = False,
    allowed_base_dir: Optional[Path | str] = None,
) -> Path:
    """Validate that a filesystem path is secure, canonical, and free of path traversal vulnerabilities.

    Rejects null bytes and checks containment within allowed_base_dir if specified.
    """
    path_str = str(path)
    if "\0" in path_str:
        raise PathSecurityError(f"Path contains illegal null bytes: {path!r}")

    cleaned = path_str.strip()
    if not cleaned:
        raise PathSecurityError("Path cannot be empty.")

    p = Path(cleaned)
    try:
        resolved = p.resolve()
    except Exception as exc:
        raise PathSecurityError(f"Failed to resolve canonical path '{path}': {exc}") from exc

    if allowed_base_dir is not None:
        base_resolved = Path(allowed_base_dir).resolve()
        try:
            resolved.relative_to(base_resolved)
        except ValueError as exc:
            raise PathSecurityError(
                f"Path '{path}' escapes allowed base directory '{allowed_base_dir}'."
            ) from exc

    if not allow_missing and not resolved.exists():
        raise VideoNotFoundError(f"Target path does not exist: '{resolved}'")

    return resolved


def generate_unique_artifact_directory(
    base_dir: Path | str,
    case_id: str,
    evidence_id: str,
) -> Path:
    """Generate a unique collision-free directory path for a forensic extraction run.

    Format: <base_dir>/<case_id>_<evidence_id>_<YYYYMMDD_HHMMSS>_<uuid8>
    """
    sanitized_case = re.sub(r"[^a-zA-Z0-9_-]", "_", case_id.strip()) or "case"
    sanitized_ev = re.sub(r"[^a-zA-Z0-9_-]", "_", evidence_id.strip()) or "evidence"
    ts_str = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
    token = uuid.uuid4().hex[:8]
    folder_name = f"{sanitized_case}_{sanitized_ev}_{ts_str}_{token}"
    return Path(base_dir).resolve() / folder_name


def get_ffmpeg_tool_metadata() -> dict[str, str]:
    """Inspect and cache local FFmpeg and FFprobe binary versions and paths."""
    global _CACHED_TOOL_METADATA
    if _CACHED_TOOL_METADATA is not None:
        return dict(_CACHED_TOOL_METADATA)

    metadata: dict[str, str] = {
        "ffmpeg_binary": shutil.which("ffmpeg") or "ffmpeg",
        "ffprobe_binary": shutil.which("ffprobe") or "ffprobe",
        "ffmpeg_version": "unknown",
        "ffprobe_version": "unknown",
    }

    try:
        res = subprocess.run(
            [metadata["ffmpeg_binary"], "-version"],
            capture_output=True,
            text=True,
            check=False,
            timeout=5.0,
        )
        if res.returncode == 0 and res.stdout:
            first_line = res.stdout.splitlines()[0]
            metadata["ffmpeg_version"] = first_line.strip()
    except Exception as exc:
        logger.warning("Failed to determine FFmpeg version: %s", exc)

    try:
        res = subprocess.run(
            [metadata["ffprobe_binary"], "-version"],
            capture_output=True,
            text=True,
            check=False,
            timeout=5.0,
        )
        if res.returncode == 0 and res.stdout:
            first_line = res.stdout.splitlines()[0]
            metadata["ffprobe_version"] = first_line.strip()
    except Exception as exc:
        logger.warning("Failed to determine FFprobe version: %s", exc)

    _CACHED_TOOL_METADATA = metadata
    return dict(metadata)


def _parse_frame_rate(fps_val: Any) -> Optional[float]:
    """Safely convert FFprobe frame rate strings (e.g. '30/1', '29.97/1', '25/1') to a positive float."""
    if fps_val is None or not isinstance(fps_val, (str, int, float)):
        return None
    if isinstance(fps_val, (int, float)):
        val = float(fps_val)
        return val if math.isfinite(val) and val > 0.0 else None

    cleaned = str(fps_val).strip()
    if not cleaned or cleaned == "0/0":
        return None

    try:
        if "/" in cleaned:
            num_str, den_str = cleaned.split("/", 1)
            num = float(num_str)
            den = float(den_str)
            if den == 0.0:
                return None
            val = num / den
            return val if math.isfinite(val) and val > 0.0 else None
        val = float(cleaned)
        return val if math.isfinite(val) and val > 0.0 else None
    except (ValueError, ZeroDivisionError):
        return None


# --- Core Forensic Video Metadata Extraction ---


def probe_video_metadata(
    video_path: Path | str,
    evidence_id: str,
    *,
    expected_sha256: Optional[str] = None,
    timeout_seconds: float = 15.0,
    allowed_base_dir: Optional[Path | str] = None,
) -> VideoMetadata:
    """Extract container and stream forensic metadata from evidence video using FFprobe.

    Guarantees:
    - Calculates and verifies SHA-256 before and after inspection to prove read-only access.
    - Captures container format, duration, dimensions, frame rate, stream details, codec, and audio/video flags.
    - Accurately identifies missing or corrupted files, raising actionable forensic errors.
    """
    resolved_path = validate_safe_path(
        video_path,
        allow_missing=False,
        allowed_base_dir=allowed_base_dir,
    )
    if not resolved_path.is_file():
        raise VideoProcessingError(f"Target evidence '{resolved_path}' is a directory, not a video file.")

    # 1. Pre-inspection cryptographic hash
    sha256_before = compute_file_sha256(resolved_path)
    if expected_sha256 is not None:
        cleaned_expected = expected_sha256.strip().lower()
        if sha256_before != cleaned_expected:
            raise VideoIntegrityError(
                f"Evidence integrity mismatch for '{resolved_path}': expected SHA-256 "
                f"{cleaned_expected}, but observed {sha256_before}."
            )

    tools = get_ffmpeg_tool_metadata()
    ffprobe_bin = tools.get("ffprobe_binary", "ffprobe")

    cmd = [
        ffprobe_bin,
        "-v",
        "error",
        "-show_format",
        "-show_streams",
        "-print_format",
        "json",
        str(resolved_path),
    ]

    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise VideoProcessingTimeoutError(
            f"FFprobe metadata extraction timed out after {timeout_seconds} seconds for '{resolved_path}'."
        ) from exc
    except Exception as exc:
        raise VideoProcessingError(f"Failed to execute FFprobe on '{resolved_path}': {exc}") from exc

    if proc.returncode != 0:
        err_msg = proc.stderr.strip() if proc.stderr else f"Exit code {proc.returncode}"
        if "Invalid data found" in err_msg or "moov atom not found" in err_msg:
            raise CorruptVideoError(f"Corrupt or unreadable video file '{resolved_path}': {err_msg}")
        raise UnsupportedVideoFormatError(
            f"FFprobe failed to inspect media format for '{resolved_path}': {err_msg}"
        )

    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise CorruptVideoError(
            f"FFprobe output for '{resolved_path}' could not be parsed as valid JSON: {exc}"
        ) from exc

    format_dict = data.get("format", {})
    streams_list = data.get("streams", [])

    if not streams_list and not format_dict:
        raise UnsupportedVideoFormatError(
            f"FFprobe returned empty format and stream metadata for '{resolved_path}'."
        )

    # Parse container format
    container_format = format_dict.get("format_name", "unknown")
    format_long_name = format_dict.get("format_long_name")

    # Duration parsing
    duration_str = format_dict.get("duration")
    duration_seconds = 0.0
    if duration_str is not None:
        try:
            dur = float(duration_str)
            if math.isfinite(dur) and dur >= 0.0:
                duration_seconds = dur
        except (ValueError, TypeError):
            duration_seconds = 0.0

    # Stream parsing
    parsed_streams: list[VideoStreamMetadata] = []
    video_streams_count = 0
    audio_streams_count = 0
    primary_video: Optional[dict[str, Any]] = None

    for s in streams_list:
        codec_type = str(s.get("codec_type", "unknown")).lower()
        if codec_type == "video":
            video_streams_count += 1
            if primary_video is None:
                primary_video = s
        elif codec_type == "audio":
            audio_streams_count += 1

        width_val = s.get("width")
        height_val = s.get("height")
        width = int(width_val) if isinstance(width_val, (int, float)) and width_val > 0 else None
        height = int(height_val) if isinstance(height_val, (int, float)) and height_val > 0 else None

        fps = _parse_frame_rate(s.get("r_frame_rate")) or _parse_frame_rate(s.get("avg_frame_rate"))

        bitrate_val = s.get("bit_rate")
        bit_rate = int(bitrate_val) if isinstance(bitrate_val, (int, str)) and str(bitrate_val).isdigit() else None

        stream_dur_val = s.get("duration")
        stream_duration: Optional[float] = None
        if stream_dur_val is not None:
            try:
                sdur = float(stream_dur_val)
                if math.isfinite(sdur) and sdur >= 0.0:
                    stream_duration = sdur
            except (ValueError, TypeError):
                stream_duration = None

        nb_frames_val = s.get("nb_frames")
        nb_frames = int(nb_frames_val) if isinstance(nb_frames_val, (int, str)) and str(nb_frames_val).isdigit() else None

        parsed_streams.append(
            VideoStreamMetadata(
                stream_index=int(s.get("index", len(parsed_streams))),
                codec_type=codec_type,
                codec_name=s.get("codec_name"),
                codec_long_name=s.get("codec_long_name"),
                profile=s.get("profile"),
                width=width,
                height=height,
                fps=fps,
                r_frame_rate=s.get("r_frame_rate"),
                avg_frame_rate=s.get("avg_frame_rate"),
                bit_rate=bit_rate,
                duration_seconds=stream_duration,
                nb_frames=nb_frames,
                tags={str(k): str(v) for k, v in s.get("tags", {}).items()},
            )
        )

    # Fall back to primary stream duration if container duration is zero
    if duration_seconds == 0.0 and primary_video is not None:
        p_dur = primary_video.get("duration")
        if p_dur:
            try:
                pd = float(p_dur)
                if math.isfinite(pd) and pd >= 0.0:
                    duration_seconds = pd
            except (ValueError, TypeError):
                pass

    # Primary video attributes
    p_width: Optional[int] = None
    p_height: Optional[int] = None
    p_fps: Optional[float] = None
    p_codec: Optional[str] = None
    if primary_video is not None:
        if primary_video.get("width"):
            p_width = int(primary_video["width"])
        if primary_video.get("height"):
            p_height = int(primary_video["height"])
        p_fps = _parse_frame_rate(primary_video.get("r_frame_rate")) or _parse_frame_rate(
            primary_video.get("avg_frame_rate")
        )
        p_codec = primary_video.get("codec_name")

    # Raw creation time tag
    container_tags = {str(k): str(v) for k, v in format_dict.get("tags", {}).items()}
    creation_time_raw = container_tags.get("creation_time")
    if not creation_time_raw and primary_video is not None:
        creation_time_raw = primary_video.get("tags", {}).get("creation_time")

    # File size in bytes
    size_val = format_dict.get("size")
    if size_val and str(size_val).isdigit():
        file_size_bytes = int(size_val)
    else:
        file_size_bytes = os.path.getsize(resolved_path)

    # 2. Post-inspection cryptographic hash to guarantee immutability
    sha256_after = compute_file_sha256(resolved_path)
    if sha256_before != sha256_after:
        raise VideoIntegrityError(
            f"Forensic integrity violated: file '{resolved_path}' was modified during metadata inspection!"
        )

    return VideoMetadata(
        container_format=container_format,
        format_long_name=format_long_name,
        duration_seconds=duration_seconds,
        file_size_bytes=file_size_bytes,
        sha256_hash=sha256_before,
        width=p_width,
        height=p_height,
        fps=p_fps,
        codec_name=p_codec,
        has_video=(video_streams_count > 0),
        has_audio=(audio_streams_count > 0),
        video_streams_count=video_streams_count,
        audio_streams_count=audio_streams_count,
        creation_time_raw=creation_time_raw,
        streams=parsed_streams,
        tags=container_tags,
        tool_version=tools.get("ffprobe_version"),
    )


# --- Core Forensic Keyframe / Interval Extraction Engine ---


def extract_video_keyframes(
    video_path: Path | str,
    output_dir: Path | str,
    evidence_id: str,
    case_id: str,
    *,
    options: Optional[VideoExtractionOptions] = None,
    expected_sha256: Optional[str] = None,
    allowed_evidence_dir: Optional[Path | str] = None,
    allowed_artifact_dir: Optional[Path | str] = None,
) -> VideoExtractionResult:
    """Extract keyframes or fixed-interval image artifacts from video evidence using FFmpeg.

    Guarantees:
    - Never modifies original evidence; computes and verifies SHA-256 pre- and post-extraction.
    - Derived frames are written into a distinct, verified artifact directory.
    - Preserves presentation timestamp (PTS) offsets separately from derived UTC timestamps.
    - Computes anchored UTC timestamps only when recording start and timezone are provided.
    - Enforces hard ceilings on frame count, time bounds, and execution timeout.
    - Cleans up only temporary derived artifacts created by the current operation in the event of failure.
    """
    start_wall_time = time.time()
    processed_at_utc = datetime.now(timezone.utc)

    # 1. Path validation & Security
    resolved_video = validate_safe_path(
        video_path,
        allow_missing=False,
        allowed_base_dir=allowed_evidence_dir,
    )
    if not resolved_video.is_file():
        raise VideoProcessingError(f"Target evidence '{resolved_video}' is not a valid video file.")

    if Path(output_dir).is_symlink():
        raise PathSecurityError(
            f"Derived artifact output directory '{output_dir}' cannot be a symlink."
        )

    resolved_output_dir = validate_safe_path(
        output_dir,
        allow_missing=True,
        allowed_base_dir=allowed_artifact_dir,
    )

    # Guard: derived artifacts must never overwrite raw evidence or its container folder
    if resolved_output_dir == resolved_video:
        raise PathSecurityError(
            f"Derived artifact output directory '{resolved_output_dir}' cannot overwrite original evidence file."
        )
    try:
        resolved_output_dir.relative_to(resolved_video.parent)
        raise PathSecurityError(
            f"Derived artifact output directory '{resolved_output_dir}' cannot be inside "
            f"or equal to the raw evidence directory '{resolved_video.parent}'."
        )
    except ValueError:
        pass

    if allowed_evidence_dir is not None:
        base_ev = Path(allowed_evidence_dir).resolve()
        try:
            resolved_output_dir.relative_to(base_ev)
            raise PathSecurityError(
                f"Derived artifact output directory '{resolved_output_dir}' cannot be inside "
                f"the permitted raw evidence directory '{base_ev}'."
            )
        except ValueError:
            pass

    # Sanitize evidence ID for filenames
    sanitized_ev_id = re.sub(r"[^a-zA-Z0-9_-]", "_", evidence_id.strip())
    if not sanitized_ev_id:
        sanitized_ev_id = "evidence"

    # 2. Options initialization & Pre-inspection
    opts = options or VideoExtractionOptions()

    if opts.create_unique_run_subdir:
        ts_str = datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")
        token = uuid.uuid4().hex[:8]
        resolved_output_dir = resolved_output_dir / f"run_{sanitized_ev_id}_{ts_str}_{token}"

    # Guard against symlinks or any pre-existing frame artifacts in output directory (overwrite forbidden)
    if resolved_output_dir.exists():
        for existing in resolved_output_dir.iterdir():
            if existing.is_symlink():
                raise PathSecurityError(
                    f"Insecure symlink detected in artifact output directory: '{existing}'. "
                    "Derived artifacts will not be written to directories containing symlinks."
                )
            if existing.is_file() and existing.name.startswith(f"{sanitized_ev_id}_frame_"):
                raise VideoExtractionError(
                    f"Pre-existing artifact collision: frame artifact '{existing.name}' already "
                    f"exists in '{resolved_output_dir}'. Overwriting derived artifacts is strictly "
                    "forbidden in YUKTI forensics. Use a unique output directory for each run."
                )

    # Probe metadata and verify video stream presence
    metadata = probe_video_metadata(
        resolved_video,
        evidence_id=evidence_id,
        expected_sha256=expected_sha256,
        timeout_seconds=opts.timeout_seconds,
        allowed_base_dir=allowed_evidence_dir,
    )

    if not metadata.has_video:
        raise UnsupportedVideoFormatError(
            f"Cannot extract frames from '{resolved_video}': media file contains no video streams."
        )

    # Validate start/end time bounds against metadata duration
    if metadata.duration_seconds > 0.0:
        if opts.start_time_seconds is not None and opts.start_time_seconds >= metadata.duration_seconds:
            raise ValueError(
                f"start_time_seconds ({opts.start_time_seconds}s) exceeds or equals video duration "
                f"({metadata.duration_seconds}s)."
            )

    # 3. Prepare artifact directory and snapshot pre-existing files for failure cleanup and immutability check
    resolved_output_dir.mkdir(parents=True, exist_ok=True)
    existing_files: set[Path] = set(resolved_output_dir.iterdir())
    existing_file_hashes: dict[Path, str] = {
        f: compute_file_sha256(f) for f in existing_files if f.is_file() and not f.is_symlink()
    }

    def _cleanup_partial_artifacts() -> None:
        """Remove only the newly created artifacts generated in this specific execution run."""
        if not resolved_output_dir.exists():
            return
        current_files = set(resolved_output_dir.iterdir())
        new_files = current_files - existing_files
        for f in new_files:
            try:
                if f.exists() or f.is_symlink():
                    f.unlink()
            except Exception as e:
                logger.warning("Failed to clean up partial frame '%s': %s", f, e)

    # 4. Construct FFmpeg command (strictly using -n to prevent overwriting any files)
    tools = get_ffmpeg_tool_metadata()
    ffmpeg_bin = tools.get("ffmpeg_binary", "ffmpeg")

    cmd: list[str] = [ffmpeg_bin, "-n"]

    # Accurate seeking with -ss and -to, preserving source presentation timestamps
    if opts.start_time_seconds is not None:
        cmd.extend(["-ss", f"{opts.start_time_seconds:.6f}"])
    if opts.end_time_seconds is not None:
        cmd.extend(["-to", f"{opts.end_time_seconds:.6f}"])

    cmd.append("-copyts")
    cmd.extend(["-i", str(resolved_video)])

    # Video filter chain
    filter_chain: list[str] = []
    if opts.keyframes_only:
        filter_chain.append("select='eq(pict_type\\,I)'")
    else:
        if opts.fps is not None:
            eff_fps = opts.fps
        elif opts.interval_seconds is not None:
            eff_fps = 1.0 / opts.interval_seconds
        else:
            eff_fps = 1.0
        filter_chain.append(f"fps={eff_fps:.6f}")

    filter_chain.append("showinfo")
    cmd.extend(["-vf", ",".join(filter_chain)])

    if opts.keyframes_only:
        cmd.extend(["-fps_mode", "vfr"])

    # Hard ceiling on frame count
    cmd.extend(["-vframes", str(opts.max_frames)])

    # Output filename template
    pattern = str(resolved_output_dir / f"{sanitized_ev_id}_frame_%06d.{opts.image_format}")
    cmd.append(pattern)

    # 5. Execute FFmpeg frame extraction subprocess
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=opts.timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        _cleanup_partial_artifacts()
        raise VideoProcessingTimeoutError(
            f"FFmpeg frame extraction timed out after {opts.timeout_seconds} seconds for '{resolved_video}'."
        ) from exc
    except Exception as exc:
        _cleanup_partial_artifacts()
        raise VideoExtractionError(f"Failed to execute FFmpeg extraction: {exc}") from exc

    # Identify all frame artifacts belonging to this run
    created_files_in_run = sorted(
        [
            p
            for p in resolved_output_dir.iterdir()
            if p.is_file() and p.name.startswith(f"{sanitized_ev_id}_frame_")
        ],
        key=lambda p: p.name,
    )

    if proc.returncode != 0:
        _cleanup_partial_artifacts()
        err_msg = proc.stderr.strip() if proc.stderr else f"Exit code {proc.returncode}"
        raise VideoExtractionError(f"FFmpeg extraction failed on '{resolved_video}': {err_msg}")

    if not created_files_in_run:
        _cleanup_partial_artifacts()
        raise VideoExtractionError(
            f"No frames were extracted from video '{resolved_video}' using options: "
            f"interval={opts.interval_seconds}, bounds=[{opts.start_time_seconds}, {opts.end_time_seconds}]."
        )

    # 6. Post-extraction cryptographic hash check to verify absolute immutability
    sha256_after = compute_file_sha256(resolved_video)
    if metadata.sha256_hash != sha256_after:
        _cleanup_partial_artifacts()
        raise VideoIntegrityError(
            f"CRITICAL FORENSIC VIOLATION: Source video '{resolved_video}' was modified during frame extraction! "
            f"Before: {metadata.sha256_hash}, After: {sha256_after}."
        )

    # 7. Parse showinfo logs to obtain precise PTS offsets for each frame
    # Pattern: [Parsed_showinfo_... @ ...] n: 0 pts: 0 pts_time:1.2345
    pts_matches = re.findall(
        r"\[Parsed_showinfo_[0-9]+ @ [0-9a-zA-Z]+\]\s+n:\s*([0-9]+)\s+pts:\s*([0-9-]+)\s+pts_time:([0-9.eE+-]+)",
        proc.stderr,
    )
    pts_by_n: dict[int, float] = {}
    for match in pts_matches:
        try:
            n_idx = int(match[0])
            val = float(match[2])
            if n_idx in pts_by_n and abs(pts_by_n[n_idx] - val) > 1e-4:
                _cleanup_partial_artifacts()
                raise VideoExtractionError(
                    f"Conflicting duplicate showinfo PTS records for frame index {n_idx}: "
                    f"{pts_by_n[n_idx]} vs {val}."
                )
            pts_by_n[n_idx] = val
        except (ValueError, IndexError):
            pass

    # Strictly verify PTS correspondence and symlink security for every extracted frame file
    for idx in range(len(created_files_in_run)):
        frame_file = created_files_in_run[idx]
        if frame_file.is_symlink():
            _cleanup_partial_artifacts()
            raise PathSecurityError(
                f"CRITICAL FORENSIC VIOLATION: Output frame '{frame_file}' is a symlink! "
                "Refusing redirected output."
            )
        if frame_file.resolve().parent != resolved_output_dir.resolve():
            _cleanup_partial_artifacts()
            raise PathSecurityError(
                f"CRITICAL FORENSIC VIOLATION: Output frame '{frame_file}' escaped output directory "
                f"'{resolved_output_dir}'!"
            )
        if idx not in pts_by_n:
            _cleanup_partial_artifacts()
            raise VideoExtractionError(
                f"Forensic PTS correspondence failure: Extracted frame #{idx + 1} "
                f"('{created_files_in_run[idx].name}') lacks an exact corresponding FFmpeg showinfo PTS record. "
                "Refusing to assign an estimated or guessed timestamp."
            )

    # Verify that pre-existing user files were never modified or corrupted during processing
    for f, orig_hash in existing_file_hashes.items():
        if not f.exists() or compute_file_sha256(f) != orig_hash:
            _cleanup_partial_artifacts()
            raise VideoIntegrityError(
                f"CRITICAL FORENSIC VIOLATION: Pre-existing user file '{f}' was modified during processing!"
            )

    # 8. Calculate anchored or unanchored timestamps and build ExtractedFrameArtifacts
    recording_start_utc: Optional[datetime] = None
    if opts.recording_start_timestamp is not None:
        recording_start_utc = parse_and_normalize_timestamp(
            opts.recording_start_timestamp,
            source_timezone=opts.source_timezone,
            clock_offset_seconds=opts.clock_offset_seconds,
        )

    derived_artifacts: list[ExtractedFrameArtifact] = []

    for idx, frame_path in enumerate(created_files_in_run):
        frame_idx = idx + 1
        artifact_id = f"artifact_{sanitized_ev_id}_frame_{frame_idx:06d}"
        pts_offset = pts_by_n[idx]

        frame_hash = compute_file_sha256(frame_path)
        frame_size = os.path.getsize(frame_path)

        frame_utc: Optional[datetime] = None
        frame_window: Optional[NormalizedTimeWindow] = None
        is_anchored = False

        if recording_start_utc is not None:
            is_anchored = True
            frame_utc = recording_start_utc + timedelta(seconds=pts_offset)
            # Create a 1-second uncertainty window [frame_utc, frame_utc + 1s)
            frame_window = NormalizedTimeWindow(
                nominal_utc=frame_utc,
                precision=CanonicalTimePrecision.EXACT_SECOND,
                raw_precision_input="EXACT_SECOND",
                window_start_utc=frame_utc,
                window_end_utc=frame_utc + timedelta(seconds=1.0),
                uncertainty_span_seconds=1.0,
                source_timezone=opts.source_timezone,
                clock_offset_seconds=opts.clock_offset_seconds,
            )

        derived_artifacts.append(
            ExtractedFrameArtifact(
                artifact_id=artifact_id,
                case_id=case_id,
                source_evidence_id=evidence_id,
                source_video_sha256=metadata.sha256_hash,
                frame_index=frame_idx,
                pts_offset_seconds=pts_offset,
                timestamp_utc=frame_utc,
                timestamp_precision=CanonicalTimePrecision.EXACT_SECOND if is_anchored else None,
                time_window=frame_window,
                is_utc_anchored=is_anchored,
                file_path=str(frame_path.resolve()),
                file_name=frame_path.name,
                image_format=opts.image_format,
                file_size_bytes=frame_size,
                sha256_hash=frame_hash,
                width=metadata.width or 0,
                height=metadata.height or 0,
                extraction_method="keyframes" if opts.keyframes_only else "fixed_interval",
            )
        )

    elapsed_processing = time.time() - start_wall_time

    return VideoExtractionResult(
        case_id=case_id,
        source_evidence_id=evidence_id,
        source_video_path=str(resolved_video.resolve()),
        source_video_sha256_before=metadata.sha256_hash,
        source_video_sha256_after=sha256_after,
        integrity_verified=(metadata.sha256_hash == sha256_after),
        output_directory=str(resolved_output_dir.resolve()),
        video_metadata=metadata,
        options_applied=opts,
        frames=derived_artifacts,
        total_frames_extracted=len(derived_artifacts),
        processing_time_seconds=round(elapsed_processing, 4),
        processed_at_utc=processed_at_utc,
        tool_metadata=tools,
        provenance_note=(
            "Derived frame artifacts are secondary derived assets; original evidence video was not modified. "
            "Each frame is hashed and bound to the source video SHA-256."
        ),
    )
