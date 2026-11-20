"""Supplied Frame Artifact Adapter (Member 4 — Phase 3F).

Validates frame artifacts that were extracted earlier (for example in a previous run) and are
supplied to a new run as optional input. A supplied frame is accepted only when all of these
hold:

- its source evidence item was validated and hash-verified in this run, and is CCTV;
- ``source_video_sha256`` equals that evidence item's recorded SHA-256, so the frame is bound to
  the same bytes;
- its identifiers, PTS offset and SHA-256 are well formed;
- it carries no host path. ``file_path`` and absolute or ``..`` paths are rejected. An optional
  ``artifact_relative_path`` must be a plain relative POSIX path.

UTC anchoring is never taken from the input (DEC-007). It is recomputed, once, from the
evidence's validated ``recording_start_timestamp``, ``source_timezone`` and
``clock_offset_seconds`` plus the frame's PTS offset, with the same rule as ``video.py``. A
supplied ``timestamp_utc`` / ``time_window`` is ignored and reported.

Frames are linked to events only by temporal alignment (same source evidence, overlapping UTC
windows). This is provenance, not semantic confirmation of what the frame shows.
"""

import math
import re
from collections.abc import Mapping
from datetime import datetime, timedelta
from pathlib import PurePosixPath
from typing import Any

from app.modules.reconstruction.adapters.diagnostics import (
    DiagnosticSeverity,
    PipelineStage,
    ReconstructionDiagnostic,
)
from app.modules.reconstruction.adapters.evidence_adapter import EvidenceRecordView
from app.modules.reconstruction.adapters.frame_handoff import (
    FrameArtifactOrigin,
    FrameArtifactReference,
)
from app.modules.reconstruction.temporal import parse_and_normalize_timestamp
from app.schemas.reconstruction import CanonicalTimePrecision, NormalizedTimeWindow

_SAFE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,191}$")
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

FRAME_REQUIRED_FIELDS: tuple[str, ...] = (
    "artifact_id",
    "source_evidence_id",
    "source_video_sha256",
    "frame_index",
    "pts_offset_seconds",
    "sha256_hash",
)
FRAME_OPTIONAL_FIELDS: tuple[str, ...] = (
    "case_id",
    "artifact_reference",
    "artifact_relative_path",
    "origin",
    "image_format",
    "width",
    "height",
    "file_size_bytes",
    "extraction_method",
)
FRAME_DERIVED_FIELDS: frozenset[str] = frozenset(
    {"timestamp_utc", "timestamp_precision", "time_window", "timestamp_utc_anchored", "is_utc_anchored"}
)
FRAME_FORBIDDEN_FIELDS: frozenset[str] = frozenset({"file_path", "output_directory", "source_video_path"})


class FrameArtifactInputError(ValueError):
    """One supplied frame artifact is invalid; it is excluded and reported."""

    code = "INVALID_FRAME_ARTIFACT"

    def __init__(self, message: str, subject_id: str) -> None:
        super().__init__(message)
        self.subject_id = subject_id


def frame_utc_window(
    evidence: EvidenceRecordView, pts_offset_seconds: float
) -> tuple[datetime, NormalizedTimeWindow] | None:
    """UTC instant and [t, t + 1 s) window of a frame, or None when it cannot be anchored."""
    settings = evidence.time_settings
    if not settings.recording_start_anchorable or settings.recording_start_timestamp is None:
        return None
    start_utc = parse_and_normalize_timestamp(
        settings.recording_start_timestamp,
        source_timezone=settings.source_timezone or "UTC",
        clock_offset_seconds=settings.effective_clock_offset_seconds,
    )
    frame_utc = start_utc + timedelta(seconds=pts_offset_seconds)
    window = NormalizedTimeWindow(
        nominal_utc=frame_utc,
        precision=CanonicalTimePrecision.EXACT_SECOND,
        raw_precision_input="EXACT_SECOND",
        window_start_utc=frame_utc,
        window_end_utc=frame_utc + timedelta(seconds=1),
        uncertainty_span_seconds=1.0,
        source_timezone=settings.source_timezone or "UTC",
        clock_offset_seconds=settings.effective_clock_offset_seconds,
    )
    return frame_utc, window


def _optional_positive_int(value: Any, name: str, issues: list[str]) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        issues.append(f"'{name}' must be a non-negative integer or null")
        return None
    number: int = value
    return number


def _optional_str(value: Any, name: str, issues: list[str]) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        issues.append(f"'{name}' must be a non-empty string or null")
        return None
    return value


def adapt_supplied_frame(
    raw: Any,
    *,
    index: int,
    case_id: str,
    evidence_by_id: Mapping[str, EvidenceRecordView],
) -> tuple[FrameArtifactReference, list[ReconstructionDiagnostic]]:
    """Validate one supplied frame artifact and recompute its UTC anchoring."""
    fallback = f"<frame_index_{index}>"
    if not isinstance(raw, Mapping):
        raise FrameArtifactInputError(
            f"Frame artifact must be a JSON object, got {type(raw).__name__}.", fallback
        )
    raw_id = raw.get("artifact_id")
    subject = raw_id if isinstance(raw_id, str) and _SAFE_ID_RE.fullmatch(raw_id) else fallback
    label = f"Frame artifact '{subject}'"

    forbidden = sorted(FRAME_FORBIDDEN_FIELDS & set(raw))
    if forbidden:
        raise FrameArtifactInputError(
            f"{label} carries host path fields {forbidden}; supplied frames must not include "
            "filesystem paths.",
            subject,
        )

    issues: list[str] = []
    missing = [name for name in FRAME_REQUIRED_FIELDS if raw.get(name) is None]
    if missing:
        issues.append(f"missing required fields {missing}")
    if raw_id is not None and subject == fallback:
        issues.append("'artifact_id' is not a safe identifier")

    source_evidence_id = raw.get("source_evidence_id")
    if source_evidence_id is not None and not isinstance(source_evidence_id, str):
        issues.append("'source_evidence_id' must be a string")
    video_sha = raw.get("source_video_sha256")
    if video_sha is not None and (not isinstance(video_sha, str) or not _SHA256_RE.fullmatch(video_sha)):
        issues.append("'source_video_sha256' must be 64 lower-case hex characters")
    frame_sha = raw.get("sha256_hash")
    if frame_sha is not None and (not isinstance(frame_sha, str) or not _SHA256_RE.fullmatch(frame_sha)):
        issues.append("'sha256_hash' must be 64 lower-case hex characters")
    frame_index = raw.get("frame_index")
    if frame_index is not None and (
        isinstance(frame_index, bool) or not isinstance(frame_index, int) or frame_index < 1
    ):
        issues.append("'frame_index' must be an integer >= 1")
    pts = raw.get("pts_offset_seconds")
    if pts is not None and (
        isinstance(pts, bool) or not isinstance(pts, (int, float)) or not math.isfinite(float(pts)) or pts < 0
    ):
        issues.append("'pts_offset_seconds' must be a finite number >= 0")

    raw_case = raw.get("case_id")
    if raw_case is not None and raw_case != case_id:
        issues.append(f"'case_id' does not match case '{case_id}'")

    rel_path = _optional_str(raw.get("artifact_relative_path"), "artifact_relative_path", issues)
    if rel_path is not None:
        posix = PurePosixPath(rel_path)
        if posix.is_absolute() or ".." in posix.parts or "\\" in rel_path or ":" in rel_path:
            issues.append("'artifact_relative_path' must be a relative POSIX path without '..'")
            rel_path = None

    width = _optional_positive_int(raw.get("width"), "width", issues)
    height = _optional_positive_int(raw.get("height"), "height", issues)
    size = _optional_positive_int(raw.get("file_size_bytes"), "file_size_bytes", issues)
    image_format = _optional_str(raw.get("image_format"), "image_format", issues)
    method = _optional_str(raw.get("extraction_method"), "extraction_method", issues)

    if issues:
        raise FrameArtifactInputError(f"{label} is malformed: {'; '.join(issues)}.", subject)

    if (
        not isinstance(source_evidence_id, str)
        or not isinstance(pts, (int, float))
        or not isinstance(frame_index, int)
    ):
        raise FrameArtifactInputError(f"{label} is malformed.", subject)
    evidence = evidence_by_id.get(source_evidence_id)
    if evidence is None:
        raise FrameArtifactInputError(
            f"{label} references evidence '{source_evidence_id}', which is not a validated and "
            "hash-verified evidence item of this run.",
            subject,
        )
    if not evidence.is_video_evidence:
        raise FrameArtifactInputError(
            f"{label} references evidence '{source_evidence_id}' of type "
            f"'{evidence.source_type}', which is not video evidence.",
            subject,
        )
    if video_sha != evidence.sha256_hash:
        raise FrameArtifactInputError(
            f"{label} is bound to video SHA-256 {video_sha}, but evidence "
            f"'{source_evidence_id}' was recorded with {evidence.sha256_hash}.",
            subject,
        )

    diagnostics: list[ReconstructionDiagnostic] = []
    derived = sorted(FRAME_DERIVED_FIELDS & set(raw))
    unknown = sorted(
        set(raw) - set(FRAME_REQUIRED_FIELDS) - set(FRAME_OPTIONAL_FIELDS) - FRAME_DERIVED_FIELDS
    )
    for code, names, text in (
        ("DERIVED_FIELDS_IGNORED", derived, "Supplied frame UTC fields were ignored and recomputed once from evidence metadata."),
        ("UNKNOWN_FRAME_FIELDS_IGNORED", unknown, "Fields outside the frame artifact contract were ignored."),
    ):
        if names:
            diagnostics.append(
                ReconstructionDiagnostic(
                    severity=DiagnosticSeverity.WARNING,
                    stage=PipelineStage.FRAME_HANDOFF,
                    code=code,
                    message=f"{text} Fields: {names}.",
                    subject_type="FRAME_ARTIFACT",
                    subject_id=subject,
                )
            )

    anchoring = frame_utc_window(evidence, float(pts))
    reference = FrameArtifactReference(
        artifact_id=subject,
        artifact_reference=f"urn:yukti:artifact:{subject}",
        origin=FrameArtifactOrigin.SUPPLIED,
        artifact_relative_path=rel_path,
        case_id=case_id,
        source_evidence_id=source_evidence_id,
        source_video_sha256=evidence.sha256_hash,
        frame_index=int(frame_index),
        pts_offset_seconds=float(pts),
        timestamp_utc_anchored=anchoring is not None,
        timestamp_utc=anchoring[0] if anchoring else None,
        timestamp_precision=CanonicalTimePrecision.EXACT_SECOND if anchoring else None,
        time_window=anchoring[1] if anchoring else None,
        sha256_hash=str(frame_sha),
        file_size_bytes=size,
        image_format=image_format,
        width=width,
        height=height,
        extraction_method=method,
    )
    return reference, diagnostics
