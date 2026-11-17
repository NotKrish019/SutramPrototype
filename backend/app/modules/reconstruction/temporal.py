"""Temporal Reconstruction Engine (Member 4 — Forensic Intelligence & System Integration).

Provides deterministic ISO 8601 timestamp normalization, IANA timezone localization,
device/DVR clock drift offset correction, precision-based and explicit half-open
uncertainty interval derivation [start, end), and overlap-aware timeline reconstruction.
"""

from copy import deepcopy
from datetime import datetime, timedelta, timezone
import math
from typing import Any, Mapping, Optional, Sequence
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from pydantic import BaseModel

from app.schemas.reconstruction import (
    CanonicalTimePrecision,
    LEGACY_TIME_PRECISION_ALIASES,
    NormalizedTimeWindow,
    TemporalOrderingStatus,
    TimelineEntry,
    TimelineOverlapPair,
    TimelineReconstructionResult,
    extract_field,
)

# Configurable default half-width (in seconds) for TimePrecision.ESTIMATED.
# Explicitly treated as an analytical configuration assumption, never a measured forensic fact.
DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS: float = 900.0


def _validate_timezone(source_timezone: Any) -> ZoneInfo:
    """Validate and resolve an IANA timezone name into a ZoneInfo instance."""
    if not isinstance(source_timezone, str) or not source_timezone.strip():
        raise ValueError(
            f"Invalid source_timezone: expected non-empty IANA timezone string, got {source_timezone!r}."
        )
    tz_name = source_timezone.strip()
    try:
        return ZoneInfo(tz_name)
    except (ZoneInfoNotFoundError, KeyError, ValueError) as exc:
        raise ValueError(f"Unrecognized IANA timezone '{source_timezone}'.") from exc


def _validate_clock_offset(clock_offset_seconds: Any) -> float:
    """Validate that clock_offset_seconds is a finite int or float (rejecting booleans and NaN/inf)."""
    if isinstance(clock_offset_seconds, bool) or not isinstance(clock_offset_seconds, (int, float)):
        raise ValueError(
            f"Invalid clock_offset_seconds: expected finite number, got {type(clock_offset_seconds).__name__}."
        )
    offset = float(clock_offset_seconds)
    if not math.isfinite(offset):
        raise ValueError(f"Invalid clock_offset_seconds: must be finite, got {clock_offset_seconds!r}.")
    return offset


def _validate_estimated_half_width(estimated_half_width_seconds: Any) -> float:
    """Validate that estimated_half_width_seconds is a finite positive number."""
    if isinstance(estimated_half_width_seconds, bool) or not isinstance(
        estimated_half_width_seconds, (int, float)
    ):
        raise ValueError(
            "Invalid estimated_half_width_seconds: expected positive finite number, "
            f"got {type(estimated_half_width_seconds).__name__}."
        )
    width = float(estimated_half_width_seconds)
    if not math.isfinite(width) or width <= 0.0:
        raise ValueError(
            f"Invalid estimated_half_width_seconds: must be finite and > 0, got {estimated_half_width_seconds!r}."
        )
    return width


def _validate_optional_duration(duration_seconds: Any) -> Optional[float]:
    """Validate optional duration_seconds without inventing a value when omitted."""
    if duration_seconds is None:
        return None
    if isinstance(duration_seconds, bool) or not isinstance(duration_seconds, (int, float)):
        raise ValueError(
            f"Invalid duration_seconds: expected non-negative finite number, got {type(duration_seconds).__name__}."
        )
    duration = float(duration_seconds)
    if not math.isfinite(duration) or duration < 0.0:
        raise ValueError(f"Invalid duration_seconds: must be finite and >= 0, got {duration_seconds!r}.")
    return duration


def normalize_time_precision(
    raw_precision: Any,
    *,
    allow_legacy_aliases: bool = True,
) -> CanonicalTimePrecision:
    """Resolve a precision string or enum to CanonicalTimePrecision.

    Supports the committed canonical values in shared/enums/README.md:
    EXACT_SECOND, MINUTE, HOUR, DAY, ESTIMATED.
    When allow_legacy_aliases=True (default), also adapts the legacy table identifiers
    from shared/schemas/README.md ('EXACT' -> EXACT_SECOND, 'APPROXIMATE' -> ESTIMATED).
    """
    if isinstance(raw_precision, CanonicalTimePrecision):
        return raw_precision
    if not isinstance(raw_precision, str) or not raw_precision.strip():
        raise ValueError(
            f"Invalid timestamp_precision: expected non-empty string, got {raw_precision!r}."
        )
    token = raw_precision.strip().upper()
    try:
        return CanonicalTimePrecision(token)
    except ValueError:
        if allow_legacy_aliases and token in LEGACY_TIME_PRECISION_ALIASES:
            return LEGACY_TIME_PRECISION_ALIASES[token]
        allowed = [p.value for p in CanonicalTimePrecision]
        raise ValueError(
            f"Unsupported timestamp_precision '{raw_precision}'. Canonical values: {allowed}."
        ) from None


def _format_utc_offset(offset: Optional[timedelta]) -> str:
    """Format a timedelta UTC offset as ±HH:MM."""
    if offset is None:
        return "UTC"
    total_seconds = int(offset.total_seconds())
    sign = "+" if total_seconds >= 0 else "-"
    total_seconds = abs(total_seconds)
    hours, rem = divmod(total_seconds, 3600)
    minutes = rem // 60
    return f"{sign}{hours:02d}:{minutes:02d}"


def _localize_naive_datetime_strictly(
    naive_dt: datetime,
    tz: ZoneInfo,
    source_timezone: str,
    raw_display: str,
) -> datetime:
    """Attach ZoneInfo(source_timezone) to a naive datetime, rejecting DST gaps and folds.

    - Detects nonexistent local times during DST spring-forward transitions (when neither
      fold=0 nor fold=1 round-trips through UTC back to the same naive wall time).
    - Detects ambiguous local times during DST fall-back transitions (when fold=0 and fold=1
      yield distinct UTC offsets that both round-trip to the same naive wall time).
    - Never silently picks fold=0 or fold=1 for ambiguous or nonexistent local timestamps.
    """
    dt_fold0 = naive_dt.replace(tzinfo=tz, fold=0)
    dt_fold1 = naive_dt.replace(tzinfo=tz, fold=1)

    rt_fold0 = dt_fold0.astimezone(timezone.utc).astimezone(tz).replace(tzinfo=None)
    rt_fold1 = dt_fold1.astimezone(timezone.utc).astimezone(tz).replace(tzinfo=None)

    if rt_fold0 != naive_dt and rt_fold1 != naive_dt:
        raise ValueError(
            f"Nonexistent naive local timestamp '{raw_display}' in timezone '{source_timezone}' "
            "(falls within a DST spring-forward transition gap). Provide an explicit ISO 8601 "
            "UTC offset to disambiguate."
        )

    offset0 = dt_fold0.utcoffset()
    offset1 = dt_fold1.utcoffset()
    if offset0 != offset1 and rt_fold0 == naive_dt and rt_fold1 == naive_dt:
        off0_str = _format_utc_offset(offset0)
        off1_str = _format_utc_offset(offset1)
        raise ValueError(
            f"Ambiguous naive local timestamp '{raw_display}' in timezone '{source_timezone}' "
            f"(occurs twice during a DST fall-back transition at UTC offsets {off0_str} and {off1_str}). "
            "Provide an explicit ISO 8601 offset to disambiguate the instant."
        )

    return dt_fold0


def _parse_iso_to_localized_datetime(
    raw_timestamp: str | datetime,
    source_timezone: str = "UTC",
    *,
    explicit_source_timezone: bool = False,
) -> datetime:
    """Parse an ISO 8601 string or datetime and localize it into the source timezone.

    - Rejects non-string/non-datetime inputs (including bare date objects, numbers, booleans, None).
    - Requires a time component ('T' or ' ') on string inputs.
    - Attaches ZoneInfo(source_timezone) when the timestamp is naive, strictly rejecting
      ambiguous (DST fall-back) and nonexistent (DST spring-forward) local wall times.
    - Converts timezone-aware timestamps into ZoneInfo(source_timezone) when an explicit
      source_timezone is provided; otherwise preserves the timestamp's explicit offset.
    """
    tz = _validate_timezone(source_timezone)
    tz_label = source_timezone.strip()

    if isinstance(raw_timestamp, datetime):
        parsed_dt = raw_timestamp
        raw_display = raw_timestamp.isoformat()
    elif isinstance(raw_timestamp, str):
        cleaned = raw_timestamp.strip()
        if not cleaned:
            raise ValueError("Invalid timestamp: timestamp string cannot be empty.")
        if "T" not in cleaned and "t" not in cleaned and " " not in cleaned:
            raise ValueError(
                f"Invalid ISO 8601 timestamp '{raw_timestamp}': missing time component."
            )
        try:
            parsed_dt = datetime.fromisoformat(cleaned)
        except ValueError as exc:
            raise ValueError(f"Malformed ISO 8601 timestamp '{raw_timestamp}': {exc}") from exc
        raw_display = cleaned
    else:
        raise ValueError(
            f"Invalid timestamp type: expected ISO 8601 str or datetime, got {type(raw_timestamp).__name__}."
        )

    if parsed_dt.tzinfo is None or parsed_dt.tzinfo.utcoffset(parsed_dt) is None:
        naive_clean = parsed_dt.replace(tzinfo=None)
        return _localize_naive_datetime_strictly(
            naive_clean,
            tz=tz,
            source_timezone=tz_label,
            raw_display=raw_display,
        )

    if explicit_source_timezone or tz_label != "UTC":
        return parsed_dt.astimezone(tz)

    return parsed_dt


def parse_and_normalize_timestamp(
    raw_timestamp: str | datetime,
    source_timezone: str = "UTC",
    clock_offset_seconds: float = 0.0,
) -> datetime:
    """Parse an ISO 8601 timestamp, localize naive timestamps via source_timezone,
    apply clock_offset_seconds, and return a timezone-aware UTC datetime.

    Formula:
        t_utc = localize(raw_timestamp, source_timezone).astimezone(UTC) + timedelta(seconds=clock_offset_seconds)
    """
    offset = _validate_clock_offset(clock_offset_seconds)
    local_dt = _parse_iso_to_localized_datetime(
        raw_timestamp,
        source_timezone=source_timezone,
    )
    utc_dt = local_dt.astimezone(timezone.utc)
    return utc_dt + timedelta(seconds=offset)


def compute_uncertainty_window(
    raw_timestamp: str | datetime,
    precision: str | CanonicalTimePrecision,
    *,
    source_timezone: str = "UTC",
    explicit_source_timezone: bool = False,
    clock_offset_seconds: float = 0.0,
    estimated_half_width_seconds: float = DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
    explicit_start: Optional[str | datetime] = None,
    explicit_end: Optional[str | datetime] = None,
) -> tuple[datetime, datetime, datetime]:
    """Derive nominal_utc and half-open uncertainty interval [window_start_utc, window_end_utc).

    Key forensic guarantees:
    1. MINUTE, HOUR, and DAY boundaries are floored and advanced in the local source_timezone
       before converting both boundaries to UTC (correctly producing 23-hour spring-forward
       and 25-hour fall-back local calendar days across DST transitions).
    2. clock_offset_seconds is applied consistently to nominal_utc, window_start_utc, and window_end_utc.
    3. All returned intervals are half-open [window_start_utc, window_end_utc) with start < end.
    """
    offset = _validate_clock_offset(clock_offset_seconds)
    offset_delta = timedelta(seconds=offset)
    canonical_precision = normalize_time_precision(precision)
    half_width_val = _validate_estimated_half_width(estimated_half_width_seconds)

    local_dt = _parse_iso_to_localized_datetime(
        raw_timestamp,
        source_timezone=source_timezone,
        explicit_source_timezone=explicit_source_timezone,
    )
    nominal_utc = local_dt.astimezone(timezone.utc) + offset_delta

    # Handle explicit uncertainty bounds if present
    if explicit_start is not None or explicit_end is not None:
        if explicit_start is None or explicit_end is None:
            raise ValueError(
                "Both explicit_start and explicit_end must be provided when specifying explicit uncertainty bounds."
            )
        start_utc = parse_and_normalize_timestamp(
            explicit_start,
            source_timezone=source_timezone,
            clock_offset_seconds=offset,
        )
        end_utc = parse_and_normalize_timestamp(
            explicit_end,
            source_timezone=source_timezone,
            clock_offset_seconds=offset,
        )
        if start_utc >= end_utc:
            raise ValueError(
                f"Invalid explicit uncertainty bounds: start ({start_utc.isoformat()}) "
                f"must be strictly earlier than end ({end_utc.isoformat()})."
            )
        if not (start_utc <= nominal_utc <= end_utc):
            raise ValueError(
                f"Nominal UTC timestamp ({nominal_utc.isoformat()}) falls outside explicit "
                f"uncertainty window [{start_utc.isoformat()}, {end_utc.isoformat()})."
            )
        return nominal_utc, start_utc, end_utc

    if canonical_precision == CanonicalTimePrecision.EXACT_SECOND:
        local_start = local_dt.replace(microsecond=0)
        start_utc = local_start.astimezone(timezone.utc) + offset_delta
        end_utc = start_utc + timedelta(seconds=1)
    elif canonical_precision == CanonicalTimePrecision.MINUTE:
        local_start = local_dt.replace(second=0, microsecond=0)
        start_utc = local_start.astimezone(timezone.utc) + offset_delta
        end_utc = start_utc + timedelta(minutes=1)
    elif canonical_precision == CanonicalTimePrecision.HOUR:
        local_start = local_dt.replace(minute=0, second=0, microsecond=0)
        start_utc = local_start.astimezone(timezone.utc) + offset_delta
        end_utc = start_utc + timedelta(hours=1)
    elif canonical_precision == CanonicalTimePrecision.DAY:
        local_start = local_dt.replace(hour=0, minute=0, second=0, microsecond=0, fold=0)
        local_end = local_start + timedelta(days=1)
        start_utc = local_start.astimezone(timezone.utc) + offset_delta
        end_utc = local_end.astimezone(timezone.utc) + offset_delta
    elif canonical_precision == CanonicalTimePrecision.ESTIMATED:
        half_delta = timedelta(seconds=half_width_val)
        start_utc = nominal_utc - half_delta
        end_utc = nominal_utc + half_delta
    else:
        raise ValueError(f"Unhandled precision value: {canonical_precision!r}")

    return nominal_utc, start_utc, end_utc


def _extract_evidence_time_settings(
    evidence_entry: Optional[Mapping[str, Any] | BaseModel | Any],
) -> tuple[str, bool, float]:
    """Extract (source_timezone, explicit_tz_flag, clock_offset_seconds) from an Evidence record or metadata dict."""
    if evidence_entry is None:
        return "UTC", False, 0.0

    metadata = extract_field(evidence_entry, "metadata", None)
    container = metadata if isinstance(metadata, Mapping) else evidence_entry

    raw_tz = extract_field(container, "source_timezone", None)
    explicit_tz = raw_tz is not None
    source_timezone = raw_tz if explicit_tz else "UTC"

    raw_offset = extract_field(container, "clock_offset_seconds", 0.0)
    if raw_offset is None:
        raw_offset = 0.0

    _validate_timezone(source_timezone)
    clock_offset = _validate_clock_offset(raw_offset)
    return source_timezone, explicit_tz, clock_offset


def normalize_event_time(
    event: Mapping[str, Any] | BaseModel | Any,
    evidence_metadata: Optional[Mapping[str, Any] | BaseModel | Any] = None,
    *,
    estimated_half_width_seconds: float = DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
) -> NormalizedTimeWindow:
    """Normalize a single Event record into a NormalizedTimeWindow without mutating inputs."""
    event_id = extract_field(event, "event_id", "<unknown_event_id>")
    try:
        raw_timestamp = extract_field(event, "timestamp", None)
        raw_precision = extract_field(event, "timestamp_precision", None)
        if raw_timestamp is None:
            raise ValueError("Missing required field 'timestamp'.")
        if raw_precision is None:
            raise ValueError("Missing required field 'timestamp_precision'.")

        source_timezone, explicit_tz, clock_offset = _extract_evidence_time_settings(
            evidence_metadata
        )
        canonical_precision = normalize_time_precision(raw_precision)
        half_width_val = _validate_estimated_half_width(estimated_half_width_seconds)
        duration_val = _validate_optional_duration(extract_field(event, "duration_seconds", None))

        explicit_start = extract_field(event, "uncertainty_window_start", None)
        explicit_end = extract_field(event, "uncertainty_window_end", None)
        has_explicit_bounds = explicit_start is not None or explicit_end is not None

        nominal_utc, start_utc, end_utc = compute_uncertainty_window(
            raw_timestamp,
            canonical_precision,
            source_timezone=source_timezone,
            explicit_source_timezone=explicit_tz,
            clock_offset_seconds=clock_offset,
            estimated_half_width_seconds=half_width_val,
            explicit_start=explicit_start,
            explicit_end=explicit_end,
        )

        span_seconds = (end_utc - start_utc).total_seconds()
        is_estimated_assumption = (
            canonical_precision == CanonicalTimePrecision.ESTIMATED and not has_explicit_bounds
        )

        return NormalizedTimeWindow(
            nominal_utc=nominal_utc,
            precision=canonical_precision,
            raw_precision_input=str(raw_precision),
            window_start_utc=start_utc,
            window_end_utc=end_utc,
            uncertainty_span_seconds=span_seconds,
            has_explicit_bounds=has_explicit_bounds,
            is_estimated_window_assumption=is_estimated_assumption,
            estimated_half_width_seconds=half_width_val if is_estimated_assumption else None,
            duration_seconds=duration_val,
            source_timezone=source_timezone,
            clock_offset_seconds=clock_offset,
        )
    except (ValueError, TypeError) as exc:
        raise ValueError(f"Event '{event_id}' timestamp normalization failed: {exc}") from exc


def intervals_overlap(
    start_a: datetime,
    end_a: datetime,
    start_b: datetime,
    end_b: datetime,
) -> Optional[tuple[datetime, datetime]]:
    """Return the half-open intersection [overlap_start, overlap_end) of [start_a, end_a) and [start_b, end_b),
    or None if they do not intersect.

    Because intervals are half-open [start, end), adjacent intervals [t0, t1) and [t1, t2)
    have max(start_a, start_b) == min(end_a, end_b) == t1 and do NOT overlap.
    """
    overlap_start = max(start_a, start_b)
    overlap_end = min(end_a, end_b)
    if overlap_start < overlap_end:
        return overlap_start, overlap_end
    return None


def reconstruct_timeline(
    events: Sequence[Mapping[str, Any] | BaseModel | Any],
    evidence_by_id: Optional[Mapping[str, Mapping[str, Any] | BaseModel | Any]] = None,
    *,
    case_id: Optional[str] = None,
    estimated_half_width_seconds: float = DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
) -> TimelineReconstructionResult:
    """Reconstruct a deterministic, uncertainty-preserving timeline from a collection of Events.

    - Never modifies input Event or Evidence objects.
    - Fails clearly with the offending event_id if any event fails validation or normalization.
    - Detects all pairwise overlapping half-open uncertainty windows [start, end).
    - Marks overlapping events with ordering_status = CONCURRENT_UNCERTAIN and populates
      concurrent_event_ids without claiming sequential order between overlapping events.
    """
    if not isinstance(events, Sequence) or isinstance(events, (str, bytes)):
        raise ValueError("Invalid events input: expected a sequence of Event records.")

    half_width_val = _validate_estimated_half_width(estimated_half_width_seconds)

    if len(events) == 0:
        return TimelineReconstructionResult(
            case_id=case_id or "",
            entries=[],
            overlaps=[],
        )

    provisional_entries: list[dict[str, Any]] = []
    seen_event_ids: set[str] = set()
    resolved_case_id: Optional[str] = case_id

    for idx, event in enumerate(events):
        raw_event_id = extract_field(event, "event_id", None)
        if not isinstance(raw_event_id, str) or not raw_event_id.strip():
            raise ValueError(f"Event at index {idx} is missing a valid string 'event_id'.")
        event_id = raw_event_id.strip()
        if event_id in seen_event_ids:
            raise ValueError(f"Duplicate event_id '{event_id}' encountered in timeline input.")
        seen_event_ids.add(event_id)

        ev_case_id = extract_field(event, "case_id", None)
        if not isinstance(ev_case_id, str) or not ev_case_id.strip():
            raise ValueError(f"Event '{event_id}' is missing a valid string 'case_id'.")
        if resolved_case_id is None:
            resolved_case_id = ev_case_id
        elif resolved_case_id != ev_case_id:
            raise ValueError(
                f"Event '{event_id}' belongs to case_id '{ev_case_id}', expected '{resolved_case_id}'."
            )

        title = extract_field(event, "title", None)
        if not isinstance(title, str) or not title.strip():
            raise ValueError(f"Event '{event_id}' is missing a valid string 'title'.")

        source_evidence_id = extract_field(event, "source_evidence_id", None)
        if not isinstance(source_evidence_id, str) or not source_evidence_id.strip():
            raise ValueError(f"Event '{event_id}' is missing a valid string 'source_evidence_id'.")

        raw_entities = extract_field(event, "involved_entity_ids", None)
        if raw_entities is None:
            involved_entity_ids: list[str] = []
        elif isinstance(raw_entities, Sequence) and not isinstance(raw_entities, (str, bytes)):
            involved_entity_ids = [str(eid) for eid in raw_entities]
        else:
            raise ValueError(
                f"Event '{event_id}' has invalid 'involved_entity_ids': expected list of strings."
            )

        location_name = extract_field(event, "location_name", None)
        raw_coords = extract_field(event, "coordinates", None)
        if isinstance(raw_coords, dict):
            coords_copy = deepcopy(raw_coords)
        elif isinstance(raw_coords, BaseModel) and hasattr(raw_coords, "lat") and hasattr(raw_coords, "lon"):
            coords_copy = {"lat": getattr(raw_coords, "lat"), "lon": getattr(raw_coords, "lon")}
        else:
            coords_copy = None

        evidence_record = (
            evidence_by_id.get(source_evidence_id)
            if isinstance(evidence_by_id, Mapping)
            else None
        )

        time_window = normalize_event_time(
            event,
            evidence_metadata=evidence_record,
            estimated_half_width_seconds=half_width_val,
        )

        provisional_entries.append(
            {
                "event_id": event_id,
                "case_id": ev_case_id,
                "title": title,
                "source_evidence_id": source_evidence_id,
                "involved_entity_ids": involved_entity_ids,
                "location_name": location_name,
                "coordinates": coords_copy,
                "time_window": time_window,
            }
        )

    # Deterministic display sort:
    # 1. window_start_utc ascending
    # 2. window_end_utc ascending (narrower intervals starting at the same instant appear first)
    # 3. nominal_utc ascending
    # 4. event_id lexicographical ascending (guarantees total order)
    provisional_entries.sort(
        key=lambda item: (
            item["time_window"].window_start_utc,
            item["time_window"].window_end_utc,
            item["time_window"].nominal_utc,
            item["event_id"],
        )
    )

    # Detect all pairwise overlapping half-open intervals [start, end)
    concurrent_map: dict[str, list[str]] = {item["event_id"]: [] for item in provisional_entries}
    overlaps: list[TimelineOverlapPair] = []

    n_entries = len(provisional_entries)
    for i in range(n_entries):
        entry_a = provisional_entries[i]
        win_a: NormalizedTimeWindow = entry_a["time_window"]
        for j in range(i + 1, n_entries):
            entry_b = provisional_entries[j]
            win_b: NormalizedTimeWindow = entry_b["time_window"]

            # Since provisional_entries are sorted by window_start_utc ascending,
            # once win_b.window_start_utc >= win_a.window_end_utc, no subsequent entry j' > j
            # can intersect win_a.
            if win_b.window_start_utc >= win_a.window_end_utc:
                break

            intersection = intervals_overlap(
                win_a.window_start_utc,
                win_a.window_end_utc,
                win_b.window_start_utc,
                win_b.window_end_utc,
            )
            if intersection is not None:
                ov_start, ov_end = intersection
                ov_seconds = (ov_end - ov_start).total_seconds()
                id_a = entry_a["event_id"]
                id_b = entry_b["event_id"]

                concurrent_map[id_a].append(id_b)
                concurrent_map[id_b].append(id_a)

                overlaps.append(
                    TimelineOverlapPair(
                        event_a_id=id_a,
                        event_b_id=id_b,
                        overlap_start_utc=ov_start,
                        overlap_end_utc=ov_end,
                        overlap_seconds=ov_seconds,
                        ordering_status=TemporalOrderingStatus.CONCURRENT_UNCERTAIN,
                    )
                )

    final_entries: list[TimelineEntry] = []
    for item in provisional_entries:
        ev_id = item["event_id"]
        concurrent_ids = sorted(concurrent_map[ev_id])
        status = (
            TemporalOrderingStatus.CONCURRENT_UNCERTAIN
            if concurrent_ids
            else TemporalOrderingStatus.STRICTLY_ORDERED
        )
        final_entries.append(
            TimelineEntry(
                event_id=ev_id,
                case_id=item["case_id"],
                title=item["title"],
                source_evidence_id=item["source_evidence_id"],
                involved_entity_ids=item["involved_entity_ids"],
                location_name=item["location_name"],
                coordinates=item["coordinates"],
                time_window=item["time_window"],
                ordering_status=status,
                concurrent_event_ids=concurrent_ids,
            )
        )

    return TimelineReconstructionResult(
        case_id=resolved_case_id or "",
        entries=final_entries,
        overlaps=overlaps,
    )
