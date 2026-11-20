"""Frame handoff boundary to Member 1, plus host-independent frame artifact references.

What Member 4 does with CCTV evidence is **frame extraction with provenance**. FFmpeg samples
still frames at fixed intervals or keyframes. Each frame is bound to the source video's
SHA-256, its PTS offset and its own SHA-256. When the evidence metadata allows it, the frame is
also anchored to UTC. **No semantic event recognition happens here.** Nothing is detected,
classified or identified in the frames, and no events are derived from them.

Semantic analysis of frames belongs to Member 1. Member 1's interface does not exist yet, so
this module defines only ``ExtractedFrameConsumer``, the protocol the pipeline hands frames to.
There is deliberately no default implementation. When no consumer is configured, the handoff
is reported as ``NOT_SUBMITTED_NO_CONSUMER``. Events that Member 1 later derives from frames
come back in through the normal event-candidate input in a later run.
"""

import os
from datetime import datetime
from enum import StrEnum
from pathlib import Path
from typing import Protocol

from pydantic import BaseModel, ConfigDict, Field

from app.schemas.reconstruction import (
    CanonicalTimePrecision,
    ExtractedFrameArtifact,
    NormalizedTimeWindow,
)

FRAME_EXTRACTION_NOTE = (
    "Frame extraction with provenance only (FFmpeg sampling bound to the source SHA-256). "
    "No semantic event recognition was performed and no events were derived from frames."
)


class FrameHandoff(BaseModel):
    """Derived frames of one CCTV evidence item, bound to the verified source SHA-256.

    This is an in-process handoff to Member 1's consumer, not a reconstruction result. Frames
    carry local file paths so that the consumer can read them.
    """

    model_config = ConfigDict(frozen=True)

    case_id: str
    source_evidence_id: str
    source_video_sha256: str
    frames: list[ExtractedFrameArtifact] = Field(default_factory=list)
    timestamps_utc_anchored: bool = Field(
        description="False when frames carry relative PTS offsets only (no UTC timestamps)."
    )
    provenance_note: str = (
        "Frames are derived artifacts from frame extraction only. Any events extracted from them "
        "must reference source_evidence_id and keep frame artifact_id / sha256_hash provenance."
    )


class ExtractedFrameConsumer(Protocol):
    """Interface that Member 1's extraction service is expected to implement."""

    def submit_frames(self, handoff: FrameHandoff) -> None:
        """Accept frames for analysis. Must not modify the frame files."""
        ...


class FrameArtifactOrigin(StrEnum):
    EXTRACTED_IN_RUN = "EXTRACTED_IN_RUN"
    SUPPLIED = "SUPPLIED"


class FrameArtifactReference(BaseModel):
    """Serializable reference to one derived frame. It never contains absolute host paths."""

    model_config = ConfigDict(frozen=True)

    artifact_id: str
    artifact_reference: str = Field(description="urn:yukti:artifact:{artifact_id}")
    origin: FrameArtifactOrigin = FrameArtifactOrigin.EXTRACTED_IN_RUN
    artifact_relative_path: str | None = Field(
        default=None,
        description=(
            "POSIX path relative to the artifact root. Each extraction writes to a new run "
            "directory (derived artifacts are never overwritten), so this value differs between "
            "runs and is excluded from result_sha256."
        ),
    )
    case_id: str
    source_evidence_id: str
    source_video_sha256: str
    frame_index: int
    pts_offset_seconds: float
    timestamp_utc_anchored: bool
    timestamp_utc: datetime | None = None
    timestamp_precision: CanonicalTimePrecision | None = None
    time_window: NormalizedTimeWindow | None = None
    sha256_hash: str
    file_size_bytes: int | None = None
    image_format: str | None = None
    width: int | None = None
    height: int | None = None
    extraction_method: str | None = None

    @classmethod
    def from_artifact(
        cls, artifact: ExtractedFrameArtifact, artifact_root: Path
    ) -> "FrameArtifactReference":
        root_real = Path(os.path.realpath(artifact_root))
        frame_real = Path(os.path.realpath(artifact.file_path))
        if not frame_real.is_relative_to(root_real):
            raise ValueError(f"Frame artifact '{artifact.artifact_id}' lies outside the artifact root.")
        return cls(
            artifact_id=artifact.artifact_id,
            artifact_reference=f"urn:yukti:artifact:{artifact.artifact_id}",
            artifact_relative_path=frame_real.relative_to(root_real).as_posix(),
            case_id=artifact.case_id,
            source_evidence_id=artifact.source_evidence_id,
            source_video_sha256=artifact.source_video_sha256,
            frame_index=artifact.frame_index,
            pts_offset_seconds=artifact.pts_offset_seconds,
            timestamp_utc_anchored=artifact.is_utc_anchored,
            timestamp_utc=artifact.timestamp_utc,
            timestamp_precision=artifact.timestamp_precision,
            time_window=artifact.time_window,
            sha256_hash=artifact.sha256_hash,
            file_size_bytes=artifact.file_size_bytes,
            image_format=artifact.image_format,
            width=artifact.width,
            height=artifact.height,
            extraction_method=artifact.extraction_method,
        )
