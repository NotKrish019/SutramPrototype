"""Evidence Storage Adapter (Member 4 — Phase 3E).

Connects shared ``Evidence`` records (``shared/schemas/README.md`` §2) to the reconstruction
engines and enforces forensic integrity.

1. **Structural validation** of the committed Evidence fields. ``case_id`` and ``evidence_id``
   must be safe path segments, because the storage key is built from them.
2. **Canonical storage keys.** Evidence bytes always live at

       data/evidence/raw/{case_id}/{evidence_id}/{evidence_id}.raw

   resolved against one configured *storage root*. The key is derived from the record's IDs.
   It is never taken from a client. ``storage_uri`` must be either that exact key or the public
   reference ``urn:yukti:evidence:{evidence_id}:storage``. Anything else is rejected:
   - client-supplied paths (absolute, relative, ``file://``) → ``CLIENT_STORAGE_PATH_REJECTED``;
   - remote URIs (``s3://``, ``https://``, ``file://host/`` …) → ``REMOTE_STORAGE_URI_REJECTED``.
     Remote staging is not implemented (Member 2 dependency).
3. **Safe resolution.** Every path component below the storage root is checked with ``lstat``.
   Symlinks anywhere below the root are rejected, and the target must be a regular file.
4. **Integrity.** SHA-256 and byte size are checked against the ingestion record, reading
   through a read-only, no-follow descriptor. Evidence is never opened for writing.
5. **DEC-006 metadata** (``source_timezone``, ``clock_offset_seconds``,
   ``recording_start_timestamp``, ``device_coordinates``) is validated and exposed as
   ``EvidenceTimeSettings``. Values that are absent stay ``None``. A missing offset or timezone
   is never reported as a verified calibration.

Results expose only the storage key (relative) and the URN. They never contain absolute host
paths. The resolved absolute path exists only in memory, on ``VerifiedEvidenceFile``, where it
is excluded from serialization.
"""

import hashlib
import os
import re
import stat
from collections.abc import Iterable, Mapping
from copy import deepcopy
from datetime import UTC, datetime
from enum import StrEnum
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

from pydantic import BaseModel, ConfigDict, Field

from app.modules.reconstruction.adapters.contracts import (
    ContractDecision,
    missing_decisions,
    normalize_approved_decisions,
)
from app.modules.reconstruction.adapters.errors import (
    ClientStoragePathRejectedError,
    EvidenceIntegrityError,
    EvidencePathSecurityError,
    EvidenceRecordError,
    EvidenceStorageError,
    RemoteStorageNotSupportedError,
)
from app.modules.reconstruction.spatial import validate_geopoint
from app.modules.reconstruction.temporal import (
    _validate_clock_offset,
    _validate_timezone,
    parse_and_normalize_timestamp,
)
from app.schemas.reconstruction import GeoPoint, VideoExtractionOptions, extract_field

# Committed EvidenceType values (shared/enums/README.md §2).
EVIDENCE_TYPES: frozenset[str] = frozenset(
    {
        "CCTV",
        "CDR",
        "CHAT_EXPORT",
        "DOCUMENT_SCAN",
        "DISK_IMAGE",
        "AUDIO_RECORDING",
        "FINANCIAL_LEDGER",
    }
)

# Evidence types whose bytes are routed through FFmpeg frame extraction.
VIDEO_EVIDENCE_TYPES: frozenset[str] = frozenset({"CCTV"})

# DEC-006 Evidence.metadata keys.
TIME_METADATA_KEYS: tuple[str, ...] = (
    "clock_offset_seconds",
    "recording_start_timestamp",
    "source_timezone",
)
DEC006_METADATA_KEYS: tuple[str, ...] = (*TIME_METADATA_KEYS, "device_coordinates")

TIME_METADATA_DECISIONS: tuple[ContractDecision, ...] = (
    ContractDecision.CLOCK_OFFSET_CONVENTION,
    ContractDecision.EVIDENCE_METADATA_KEYS,
)

STORAGE_KEY_PREFIX = "data/evidence/raw"

# Interim safeguard (Phase 3D.2). Member 2 ingestion (``app/modules/ingestion/service.py``)
# currently writes ``source_timezone="UTC"`` and ``clock_offset_seconds=0.0`` into every Evidence
# record, whether or not they were declared, and always writes ``content_detection_reason``.
# On such records these exact values cannot be told apart from a genuine declaration, so they
# are treated as undeclared (unknown), never as a UTC clock with verified zero drift. Remove this
# once Member 2 stops writing undeclared defaults
# (``docs/member-4/member2-ingestion-clock-defaults-patch.md``).
MEMBER2_INGESTION_MARKER_KEY = "content_detection_reason"
MEMBER2_UNDECLARED_TIME_DEFAULTS: dict[str, object] = {
    "source_timezone": "UTC",
    "clock_offset_seconds": 0.0,
}
STORAGE_URN_PREFIX = "urn:yukti:evidence:"
STORAGE_URN_SUFFIX = ":storage"

REMOTE_STAGING_NOTICE = (
    "Remote evidence staging is not implemented. The ingestion service (Member 2) must stage "
    "the bytes at the canonical storage key under the configured storage root."
)

KNOWN_REMOTE_SCHEMES: frozenset[str] = frozenset(
    {
        "s3",
        "s3a",
        "gs",
        "gcs",
        "az",
        "azure",
        "abfs",
        "abfss",
        "wasb",
        "wasbs",
        "http",
        "https",
        "ftp",
        "ftps",
        "sftp",
        "ssh",
        "smb",
        "nfs",
        "hdfs",
        "minio",
    }
)

_SAFE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
_SHA256_RE = re.compile(r"^[0-9a-fA-F]{64}$")
_HASH_CHUNK_BYTES = 1 << 20


class ClockCalibrationStatus(StrEnum):
    """How ``clock_offset_seconds`` was obtained. No value means *independently verified*."""

    DECLARED_NOT_INDEPENDENTLY_VERIFIED = "DECLARED_NOT_INDEPENDENTLY_VERIFIED"
    NOT_DECLARED_UNCALIBRATED = "NOT_DECLARED_UNCALIBRATED"
    NOT_APPLIED_DECISION_WITHHELD = "NOT_APPLIED_DECISION_WITHHELD"


def storage_key_for(case_id: str, evidence_id: str) -> str:
    """Canonical storage key, relative to the configured storage root."""
    return f"{STORAGE_KEY_PREFIX}/{case_id}/{evidence_id}/{evidence_id}.raw"


def storage_reference_for(evidence_id: str) -> str:
    """Public, host-independent reference to an evidence item's stored bytes."""
    return f"{STORAGE_URN_PREFIX}{evidence_id}{STORAGE_URN_SUFFIX}"


class EvidenceTimeSettings(BaseModel):
    """Validated DEC-006 temporal settings of one evidence item.

    ``source_timezone`` and ``clock_offset_seconds`` stay ``None`` when they were not declared.
    The engines then fall back to UTC and an offset of 0. That fallback is reported through
    ``clock_calibration`` and through the event's ``timezone_basis``, so it is never shown as
    a calibration.
    """

    model_config = ConfigDict(frozen=True)

    source_timezone: str | None = None
    clock_offset_seconds: float | None = None
    recording_start_timestamp: str | None = None
    recording_start_is_naive: bool = False
    applied_from_metadata: bool = Field(
        default=False,
        description="True only when DEC-001 and DEC-006 are in effect and metadata was applied.",
    )
    ignored_metadata_keys: list[str] = Field(
        default_factory=list,
        description="Time metadata keys present but not applied because a decision is withheld.",
    )
    ingestion_default_keys_ignored: list[str] = Field(
        default_factory=list,
        description=(
            "Time metadata keys whose values match Member 2's undeclared ingestion defaults "
            "and were treated as unknown."
        ),
    )

    @property
    def clock_calibration(self) -> ClockCalibrationStatus:
        if not self.applied_from_metadata:
            return ClockCalibrationStatus.NOT_APPLIED_DECISION_WITHHELD
        if self.clock_offset_seconds is None:
            return ClockCalibrationStatus.NOT_DECLARED_UNCALIBRATED
        return ClockCalibrationStatus.DECLARED_NOT_INDEPENDENTLY_VERIFIED

    @property
    def effective_clock_offset_seconds(self) -> float:
        return self.clock_offset_seconds if self.clock_offset_seconds is not None else 0.0

    @property
    def recording_start_anchorable(self) -> bool:
        """A naive recording start can be anchored to UTC only with a declared timezone."""
        if not self.applied_from_metadata or self.recording_start_timestamp is None:
            return False
        return not (self.recording_start_is_naive and self.source_timezone is None)

    def as_temporal_evidence_record(self) -> dict[str, Any]:
        """Minimal evidence record understood by ``temporal.normalize_event_time``.

        Only validated settings are passed, so the engine can never pick up unvalidated
        metadata from the raw Evidence record.
        """
        metadata: dict[str, Any] = {"clock_offset_seconds": self.effective_clock_offset_seconds}
        if self.source_timezone is not None:
            metadata["source_timezone"] = self.source_timezone
        return {"metadata": metadata}


class EvidenceRecordView(BaseModel):
    """Validated, immutable view over a shared Evidence record."""

    model_config = ConfigDict(frozen=True)

    evidence_id: str
    case_id: str
    source_type: str
    original_filename: str | None = None
    file_size_bytes: int = Field(ge=0)
    sha256_hash: str = Field(description="Lower-case SHA-256 recorded at ingestion.")
    storage_uri: str = Field(description="storage_uri exactly as supplied on the record.")
    storage_key: str = Field(description="Canonical key relative to the configured storage root.")
    storage_reference: str = Field(description="Public URN for the stored evidence bytes.")
    device_coordinates: GeoPoint | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)
    time_settings: EvidenceTimeSettings

    @property
    def is_video_evidence(self) -> bool:
        return self.source_type in VIDEO_EVIDENCE_TYPES


class VerifiedEvidenceFile(BaseModel):
    """Proof that the bytes at the canonical storage key match the ingestion record."""

    model_config = ConfigDict(frozen=True)

    evidence_id: str
    storage_key: str
    storage_reference: str
    sha256_verified: str
    file_size_bytes: int = Field(ge=0)
    verified_at_utc: datetime
    resolved_path: Path = Field(
        exclude=True, description="In-memory only; never serialized into results."
    )


def _require_str(value: Any, field_name: str, subject_id: str | None) -> str:
    if not isinstance(value, str) or not value.strip():
        raise EvidenceRecordError(
            f"Evidence '{subject_id or '<unknown>'}' is missing a valid string '{field_name}'.",
            subject_id=subject_id,
        )
    return value.strip()


def _require_safe_id(value: str, field_name: str, subject_id: str | None) -> str:
    if not _SAFE_ID_RE.fullmatch(value):
        raise EvidenceRecordError(
            f"Evidence '{subject_id or '<unknown>'}' {field_name} is not a safe storage path "
            "segment (allowed: letters, digits, '.', '_', '-'; must start with a letter or digit; "
            "at most 128 characters).",
            subject_id=subject_id,
        )
    return value


def _is_naive_iso(value: str) -> bool:
    parsed = datetime.fromisoformat(value.strip())
    return parsed.tzinfo is None or parsed.utcoffset() is None


def _extract_time_settings(
    evidence_id: str,
    metadata: Mapping[str, Any],
    approved: frozenset[ContractDecision],
) -> EvidenceTimeSettings:
    present = sorted(k for k in TIME_METADATA_KEYS if metadata.get(k) is not None)
    if missing_decisions(approved, *TIME_METADATA_DECISIONS):
        return EvidenceTimeSettings(applied_from_metadata=False, ignored_metadata_keys=present)

    raw_tz = metadata.get("source_timezone")
    raw_offset = metadata.get("clock_offset_seconds")
    raw_start = metadata.get("recording_start_timestamp")
    defaulted = _member2_default_time_keys(metadata)
    if "source_timezone" in defaulted:
        raw_tz = None
    if "clock_offset_seconds" in defaulted:
        raw_offset = None
    try:
        source_timezone: str | None = None
        if raw_tz is not None:
            if not isinstance(raw_tz, str):
                raise TypeError("source_timezone must be an IANA timezone string.")
            source_timezone = raw_tz.strip()
            _validate_timezone(source_timezone)
        offset = None if raw_offset is None else _validate_clock_offset(raw_offset)
        start_is_naive = False
        if raw_start is not None:
            if not isinstance(raw_start, str):
                raise TypeError("recording_start_timestamp must be an ISO 8601 string.")
            parse_and_normalize_timestamp(raw_start, source_timezone=source_timezone or "UTC")
            start_is_naive = _is_naive_iso(raw_start)
    except (TypeError, ValueError) as exc:
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' has invalid time metadata: {exc}", subject_id=evidence_id
        ) from exc

    return EvidenceTimeSettings(
        source_timezone=source_timezone,
        clock_offset_seconds=offset,
        recording_start_timestamp=raw_start,
        recording_start_is_naive=start_is_naive,
        applied_from_metadata=True,
        ignored_metadata_keys=[],
        ingestion_default_keys_ignored=defaulted,
    )


def _member2_default_time_keys(metadata: Mapping[str, Any]) -> list[str]:
    """Keys holding Member 2's undeclared defaults on a Member 2 ingestion record."""
    if MEMBER2_INGESTION_MARKER_KEY not in metadata:
        return []
    out: list[str] = []
    for key, default in MEMBER2_UNDECLARED_TIME_DEFAULTS.items():
        value = metadata.get(key)
        if isinstance(value, bool) or value is None:
            continue
        if isinstance(default, str) and value == default:
            out.append(key)
        elif isinstance(default, float) and isinstance(value, (int, float)) and value == default:
            out.append(key)
    return sorted(out)


def _describe_uri_kind(uri: str) -> str:
    if uri.startswith("/") or re.match(r"^[A-Za-z]:[\\/]", uri):
        return "an absolute filesystem path"
    if uri.lower().startswith("file:"):
        return "a file:// URI"
    if uri.lower().startswith(STORAGE_URN_PREFIX):
        return "a storage URN for a different evidence_id"
    return "a client-supplied relative path"


def classify_storage_uri(storage_uri: str, *, case_id: str, evidence_id: str) -> str:
    """Return the canonical storage key if ``storage_uri`` is acceptable, else raise.

    Accepted values: the canonical key for (case_id, evidence_id), or this evidence's URN.
    The rejection message never echoes the supplied value, because it may be a host path.
    """
    label = f"Evidence '{evidence_id}'"
    uri = storage_uri.strip()
    key = storage_key_for(case_id, evidence_id)
    if uri in (key, storage_reference_for(evidence_id)):
        return key
    if "\x00" in uri:
        raise EvidencePathSecurityError(
            f"{label} storage_uri contains NUL bytes.", subject_id=evidence_id
        )

    parts = urlsplit(uri)
    scheme = parts.scheme.lower()
    is_remote_file = scheme == "file" and parts.netloc not in ("", "localhost")
    if scheme in KNOWN_REMOTE_SCHEMES or is_remote_file or (scheme and parts.netloc and scheme != "file"):
        raise RemoteStorageNotSupportedError(
            f"{label} storage_uri is a remote URI (scheme '{scheme}'). {REMOTE_STAGING_NOTICE}",
            subject_id=evidence_id,
        )
    raise ClientStoragePathRejectedError(
        f"{label} storage_uri is {_describe_uri_kind(uri)}. Client-supplied storage paths are "
        f"not accepted. Expected the canonical key '{key}' or the reference "
        f"'{storage_reference_for(evidence_id)}'.",
        subject_id=evidence_id,
    )


def parse_evidence_record(
    record: Mapping[str, Any] | BaseModel | Any,
    *,
    case_id: str,
    approved_decisions: Iterable[ContractDecision | str] | None = None,
) -> EvidenceRecordView:
    """Validate a shared Evidence record (mapping or object) without mutating it.

    ``storage_uri`` is only checked for presence here. ``verify_evidence_file`` classifies it.
    """
    approved = normalize_approved_decisions(approved_decisions)
    if record is None:
        raise EvidenceRecordError("Evidence record is None.")

    evidence_id = _require_str(extract_field(record, "evidence_id"), "evidence_id", None)
    _require_safe_id(evidence_id, "evidence_id", evidence_id)
    record_case_id = _require_str(extract_field(record, "case_id"), "case_id", evidence_id)
    if record_case_id != case_id:
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' belongs to case '{record_case_id}', expected '{case_id}'.",
            subject_id=evidence_id,
        )
    _require_safe_id(record_case_id, "case_id", evidence_id)

    source_type = _require_str(extract_field(record, "source_type"), "source_type", evidence_id)
    if source_type not in EVIDENCE_TYPES:
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' has unsupported source_type '{source_type}'. "
            f"Committed EvidenceType values: {sorted(EVIDENCE_TYPES)}.",
            subject_id=evidence_id,
        )

    raw_size = extract_field(record, "file_size_bytes")
    if isinstance(raw_size, bool) or not isinstance(raw_size, int) or raw_size < 0:
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' has invalid file_size_bytes: expected non-negative int.",
            subject_id=evidence_id,
        )

    raw_hash = extract_field(record, "sha256_hash")
    if not isinstance(raw_hash, str) or not _SHA256_RE.fullmatch(raw_hash.strip()):
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' has invalid sha256_hash: expected 64 hexadecimal characters.",
            subject_id=evidence_id,
        )

    storage_uri = _require_str(extract_field(record, "storage_uri"), "storage_uri", evidence_id)

    original_filename = extract_field(record, "original_filename")
    if original_filename is not None and not isinstance(original_filename, str):
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' has non-string original_filename.", subject_id=evidence_id
        )

    raw_metadata = extract_field(record, "metadata")
    if raw_metadata is None:
        metadata: dict[str, Any] = {}
    elif isinstance(raw_metadata, Mapping):
        metadata = deepcopy(dict(raw_metadata))
    else:
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' has non-mapping metadata.", subject_id=evidence_id
        )

    try:
        device_coordinates = validate_geopoint(metadata.get("device_coordinates"))
    except ValueError as exc:
        raise EvidenceRecordError(
            f"Evidence '{evidence_id}' has invalid device_coordinates: {exc}",
            subject_id=evidence_id,
        ) from exc

    return EvidenceRecordView(
        evidence_id=evidence_id,
        case_id=record_case_id,
        source_type=source_type,
        original_filename=original_filename,
        file_size_bytes=raw_size,
        sha256_hash=raw_hash.strip().lower(),
        storage_uri=storage_uri,
        storage_key=storage_key_for(record_case_id, evidence_id),
        storage_reference=storage_reference_for(evidence_id),
        device_coordinates=device_coordinates,
        metadata=metadata,
        time_settings=_extract_time_settings(evidence_id, metadata, approved),
    )


def _resolve_storage_root(storage_root: Path | str) -> Path:
    """Return the real path of an existing storage root directory."""
    root_str = str(storage_root)
    if "\x00" in root_str or not root_str.strip():
        raise EvidencePathSecurityError("Configured storage root is empty or contains NUL bytes.")
    real = Path(os.path.realpath(root_str))
    if not real.is_dir():
        raise EvidenceStorageError("Configured storage root is not an existing directory.")
    return real


def resolve_evidence_storage_path(
    evidence: EvidenceRecordView,
    storage_root: Path | str,
) -> Path:
    """Resolve the canonical storage key of ``evidence`` to a real path inside ``storage_root``.

    Raises ``ClientStoragePathRejectedError``, ``RemoteStorageNotSupportedError``,
    ``EvidencePathSecurityError`` or ``EvidenceStorageError``.
    """
    subject = evidence.evidence_id
    label = f"Evidence '{subject}'"
    key = classify_storage_uri(
        evidence.storage_uri, case_id=evidence.case_id, evidence_id=evidence.evidence_id
    )
    root_real = _resolve_storage_root(storage_root)

    current = root_real
    st: os.stat_result | None = None
    for part in key.split("/"):
        current = current / part
        try:
            st = os.lstat(current)
        except FileNotFoundError:
            raise EvidenceStorageError(
                f"{label} bytes not found at storage key '{key}' under the configured storage root.",
                subject_id=subject,
            ) from None
        except OSError as exc:
            raise EvidenceStorageError(
                f"{label} storage key '{key}' could not be inspected: {exc.strerror}.",
                subject_id=subject,
            ) from exc
        if stat.S_ISLNK(st.st_mode):
            raise EvidencePathSecurityError(
                f"{label} storage key '{key}' crosses a symlink at component '{part}'; symlinks "
                "below the storage root are rejected.",
                subject_id=subject,
            )

    if st is None or not stat.S_ISREG(st.st_mode):
        raise EvidenceStorageError(
            f"{label} storage key '{key}' is not a regular file.", subject_id=subject
        )

    final_real = Path(os.path.realpath(current))
    if not final_real.is_relative_to(root_real):
        raise EvidencePathSecurityError(
            f"{label} storage key '{key}' escapes the configured storage root.",
            subject_id=subject,
        )
    return final_real


def verify_evidence_file(
    evidence: EvidenceRecordView,
    storage_root: Path | str,
) -> VerifiedEvidenceFile:
    """Resolve, open read-only and verify SHA-256 and size of the evidence bytes. Never writes."""
    resolved = resolve_evidence_storage_path(evidence, storage_root)
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0)
    try:
        fd = os.open(resolved, flags)
    except OSError as exc:
        raise EvidencePathSecurityError(
            f"Evidence '{evidence.evidence_id}' could not be opened read-only without following "
            f"symlinks: {exc.strerror}.",
            subject_id=evidence.evidence_id,
        ) from exc

    hasher = hashlib.sha256()
    bytes_read = 0
    with os.fdopen(fd, "rb") as handle:
        fst = os.fstat(handle.fileno())
        if not stat.S_ISREG(fst.st_mode):
            raise EvidenceStorageError(
                f"Evidence '{evidence.evidence_id}' is not a regular file.",
                subject_id=evidence.evidence_id,
            )
        while chunk := handle.read(_HASH_CHUNK_BYTES):
            hasher.update(chunk)
            bytes_read += len(chunk)

    observed = hasher.hexdigest()
    if bytes_read != evidence.file_size_bytes:
        raise EvidenceIntegrityError(
            f"Evidence '{evidence.evidence_id}' size mismatch: recorded "
            f"{evidence.file_size_bytes} bytes, observed {bytes_read} bytes.",
            subject_id=evidence.evidence_id,
        )
    if observed != evidence.sha256_hash:
        raise EvidenceIntegrityError(
            f"Evidence '{evidence.evidence_id}' SHA-256 mismatch: recorded "
            f"{evidence.sha256_hash}, observed {observed}.",
            subject_id=evidence.evidence_id,
        )

    return VerifiedEvidenceFile(
        evidence_id=evidence.evidence_id,
        storage_key=evidence.storage_key,
        storage_reference=evidence.storage_reference,
        sha256_verified=observed,
        file_size_bytes=bytes_read,
        verified_at_utc=datetime.now(UTC),
        resolved_path=resolved,
    )


def evidence_to_video_extraction_options(
    evidence: EvidenceRecordView,
    *,
    base_options: VideoExtractionOptions | None = None,
) -> VideoExtractionOptions:
    """Build per-evidence ``VideoExtractionOptions`` from validated evidence metadata.

    Sampling settings come from ``base_options``. UTC anchoring always comes from the evidence
    record (never from ``base_options``), and only when the recording start can be anchored:
    the decisions are in effect, and a naive start has a declared timezone. The clock
    correction is passed through unchanged, so ``video.py`` applies it exactly once.
    """
    if not evidence.is_video_evidence:
        raise EvidenceRecordError(
            f"Evidence '{evidence.evidence_id}' has source_type '{evidence.source_type}', "
            f"which is not supported video evidence ({sorted(VIDEO_EVIDENCE_TYPES)}).",
            subject_id=evidence.evidence_id,
        )
    base = base_options or VideoExtractionOptions(create_unique_run_subdir=True)
    data = base.model_dump()
    settings = evidence.time_settings
    if settings.recording_start_anchorable:
        data["recording_start_timestamp"] = settings.recording_start_timestamp
        data["source_timezone"] = settings.source_timezone or "UTC"
        data["clock_offset_seconds"] = settings.effective_clock_offset_seconds
    else:
        data["recording_start_timestamp"] = None
        data["source_timezone"] = "UTC"
        data["clock_offset_seconds"] = 0.0
    return VideoExtractionOptions(**data)
