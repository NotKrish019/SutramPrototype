"""Spatial Reconstruction Engine (Member 4 — Forensic Intelligence & System Integration).

Provides WGS84 coordinate validation, Haversine great-circle distance computation,
entity-grouped spatial-temporal trajectory construction, conservative elapsed-time
and minimum-required-speed calculations over half-open intervals [start, end),
and explicit handling of overlapping/uncertain event windows.
"""

from datetime import datetime
import math
from typing import Any, Mapping, Optional, Sequence

from pydantic import BaseModel, ValidationError

from app.modules.reconstruction.temporal import (
    DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
     _validate_estimated_half_width,
    intervals_overlap,
    reconstruct_timeline,
)
from app.schemas.reconstruction import (
    EntityTrajectory,
    GeoPoint,
    MovementSegment,
    NormalizedTimeWindow,
    SpatialReconstructionResult,
    SpatialTemporalOverlap,
    SpeedThresholdEvaluation,
    TemporalOrderingStatus,
    TimelineEntry,
    TransitTimeMetrics,
    extract_field,
)

# IUGG / WGS84 mean spherical Earth radius in kilometers.
EARTH_MEAN_RADIUS_KM: float = 6371.0088

# Default reference speed threshold in km/h.
# Treated strictly as a configurable analytical assumption, never a universal fact or probability.
DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH: float = 180.0


def _validate_speed_threshold(speed_threshold_kmh: Any) -> float:
    """Validate that speed_threshold_kmh is a finite positive number (rejecting bool and NaN/inf)."""
    if isinstance(speed_threshold_kmh, bool) or not isinstance(
        speed_threshold_kmh, (int, float)
    ):
        raise ValueError(
            "Invalid speed_threshold_kmh: expected positive finite number, "
            f"got {type(speed_threshold_kmh).__name__}."
        )
    threshold = float(speed_threshold_kmh)
    if not math.isfinite(threshold) or threshold <= 0.0:
        raise ValueError(
            f"Invalid speed_threshold_kmh: must be finite and > 0, got {speed_threshold_kmh!r}."
        )
    return threshold


def _validate_distance_km(distance_km: Any) -> float:
    """Validate that distance_km is a finite non-negative number."""
    if isinstance(distance_km, bool) or not isinstance(distance_km, (int, float)):
        raise ValueError(
            f"Invalid distance_km: expected non-negative finite number, got {type(distance_km).__name__}."
        )
    dist = float(distance_km)
    if not math.isfinite(dist) or dist < 0.0:
        raise ValueError(
            f"Invalid distance_km: must be finite and >= 0, got {distance_km!r}."
        )
    return 0.0 if dist == 0.0 else dist


def validate_geopoint(raw_coords: Any) -> Optional[GeoPoint]:
    """Validate and normalize a coordinate mapping or model into a frozen GeoPoint.

    - Returns None when raw_coords is None.
    - Enforces WGS84 bounds: latitude in [-90.0, 90.0], longitude in [-180.0, 180.0].
    - Rejects non-finite values (NaN, ±inf), booleans, strings, positional sequences,
      missing keys, or extra unexpected keys.
    - Never silently swaps latitude and longitude.
    """
    if raw_coords is None:
        return None

    if isinstance(raw_coords, GeoPoint):
        return raw_coords

    if isinstance(raw_coords, BaseModel):
        dumped = raw_coords.model_dump()
        if set(dumped.keys()) != {"lat", "lon"}:
            raise ValueError(
                f"Malformed coordinates model: expected exact keys {{'lat', 'lon'}}, got {set(dumped.keys())}."
            )
        try:
            return GeoPoint(lat=dumped["lat"], lon=dumped["lon"])
        except ValidationError as exc:
            raise ValueError(f"Invalid WGS84 coordinates: {exc}") from exc

    if isinstance(raw_coords, Mapping):
        keys = set(raw_coords.keys())
        if keys != {"lat", "lon"}:
            raise ValueError(
                f"Malformed coordinates mapping: expected exact keys {{'lat', 'lon'}}, got {sorted(keys)}."
            )
        try:
            return GeoPoint(lat=raw_coords["lat"], lon=raw_coords["lon"])
        except ValidationError as exc:
            raise ValueError(f"Invalid WGS84 coordinates: {exc}") from exc

    raise ValueError(
        "Invalid coordinates type: expected Mapping with 'lat' and 'lon' or GeoPoint, "
        f"got {type(raw_coords).__name__}."
    )


def haversine_distance_km(
    origin: GeoPoint | Mapping[str, Any] | BaseModel,
    destination: GeoPoint | Mapping[str, Any] | BaseModel,
) -> float:
    """Calculate the great-circle distance in kilometers (km) between two WGS84 points.

    Uses the Haversine formula with Earth mean radius R = 6371.0088 km.
    Inputs are validated as decimal degrees (lat in [-90, 90], lon in [-180, 180]).
    """
    origin_pt = validate_geopoint(origin)
    dest_pt = validate_geopoint(destination)
    if origin_pt is None or dest_pt is None:
        raise ValueError("Cannot compute Haversine distance when origin or destination is None.")

    if origin_pt.lat == dest_pt.lat and (
        origin_pt.lon == dest_pt.lon
        or (abs(origin_pt.lon) == 180.0 and abs(dest_pt.lon) == 180.0)
        or abs(origin_pt.lat) == 90.0
    ):
        return 0.0

    phi1 = math.radians(origin_pt.lat)
    phi2 = math.radians(dest_pt.lat)
    delta_phi = math.radians(dest_pt.lat - origin_pt.lat)
    delta_lambda = math.radians(dest_pt.lon - origin_pt.lon)

    sin_half_dphi = math.sin(delta_phi / 2.0)
    sin_half_dlambda = math.sin(delta_lambda / 2.0)

    a = (
        sin_half_dphi * sin_half_dphi
        + math.cos(phi1) * math.cos(phi2) * sin_half_dlambda * sin_half_dlambda
    )
    # Clamp to [0.0, 1.0] to guard against floating-point inaccuracies near antipodes.
    clamped_a = min(1.0, max(0.0, a))
    central_angle = 2.0 * math.asin(math.sqrt(clamped_a))
    distance = EARTH_MEAN_RADIUS_KM * central_angle

    if abs(distance) < 1e-12:
        return 0.0
    return distance


def calculate_transit_metrics(
    origin_time_window: NormalizedTimeWindow,
    destination_time_window: NormalizedTimeWindow,
    distance_km: float,
    *,
    speed_threshold_kmh: float = DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
) -> TransitTimeMetrics:
    """Compute elapsed-time bounds and conservative minimum required speed (km/h).

    Elapsed time definitions over half-open intervals [A_start, A_end) and [B_start, B_end):
    - nominal_elapsed_seconds = (B_nominal - A_nominal).total_seconds()
    - min_possible_elapsed_seconds = (B_start - A_end).total_seconds()
    - max_possible_elapsed_seconds = (B_end - A_start).total_seconds()

    Conservative minimum required speed:
    - Uses max_possible_elapsed_seconds (the largest time separation permitted by both
      uncertainty windows) so speed is never overstated due to timestamp uncertainty.
    - Safely handles zero or negative elapsed times without division-by-zero errors.
    """
    if not isinstance(origin_time_window, NormalizedTimeWindow) or not isinstance(
        destination_time_window, NormalizedTimeWindow
    ):
        raise ValueError(
            "origin_time_window and destination_time_window must be NormalizedTimeWindow instances."
        )

    dist = _validate_distance_km(distance_km)
    threshold = _validate_speed_threshold(speed_threshold_kmh)

    nominal_elapsed = (
        destination_time_window.nominal_utc - origin_time_window.nominal_utc
    ).total_seconds()
    min_elapsed = (
        destination_time_window.window_start_utc - origin_time_window.window_end_utc
    ).total_seconds()
    max_elapsed = (
        destination_time_window.window_end_utc - origin_time_window.window_start_utc
    ).total_seconds()

    # Nominal speed calculation (safe against zero/negative nominal_elapsed)
    if dist == 0.0 and nominal_elapsed >= 0.0:
        nominal_speed: Optional[float] = 0.0
    elif nominal_elapsed > 0.0:
        nominal_speed = (dist * 3600.0) / nominal_elapsed
    else:
        nominal_speed = None

    # Conservative minimum required speed calculation using max_possible_elapsed_seconds
    if dist == 0.0:
        min_req_speed: Optional[float] = 0.0
        exceeds_threshold = False
        evaluation = SpeedThresholdEvaluation.ZERO_DISTANCE_STATIONARY
    elif max_elapsed <= 0.0:
        min_req_speed = None
        exceeds_threshold = True
        evaluation = SpeedThresholdEvaluation.NON_POSITIVE_ELAPSED_TIME
    else:
        min_req_speed = (dist * 3600.0) / max_elapsed
        if min_req_speed > threshold:
            exceeds_threshold = True
            evaluation = SpeedThresholdEvaluation.EXCEEDS_CONFIGURED_THRESHOLD
        else:
            exceeds_threshold = False
            evaluation = SpeedThresholdEvaluation.WITHIN_CONFIGURED_THRESHOLD

    return TransitTimeMetrics(
        distance_km=dist,
        nominal_elapsed_seconds=nominal_elapsed,
        min_possible_elapsed_seconds=min_elapsed,
        max_possible_elapsed_seconds=max_elapsed,
        min_required_speed_kmh=min_req_speed,
        nominal_speed_kmh=nominal_speed,
        configured_speed_threshold_kmh=threshold,
        exceeds_configured_speed_threshold=exceeds_threshold,
        threshold_evaluation=evaluation,
    )


def create_movement_segment(
    origin_entry: TimelineEntry,
    destination_entry: TimelineEntry,
    entity_id: str,
    *,
    speed_threshold_kmh: float = DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
    intervening_unlocated_event_ids: Optional[Sequence[str]] = None,
) -> MovementSegment:
    """Construct a directed MovementSegment between two strictly ordered located TimelineEntries.

    Raises ValueError if:
    - Either entry is missing valid WGS84 coordinates.
    - The temporal uncertainty windows [start, end) overlap or origin is not strictly before destination.
    """
    if not isinstance(entity_id, str) or not entity_id.strip():
        raise ValueError("Invalid entity_id: expected non-empty string.")
    clean_entity_id = entity_id.strip()

    origin_coords = validate_geopoint(origin_entry.coordinates)
    dest_coords = validate_geopoint(destination_entry.coordinates)
    if origin_coords is None or dest_coords is None:
        raise ValueError(
            f"Cannot build MovementSegment between '{origin_entry.event_id}' and "
            f"'{destination_entry.event_id}': both events must have valid coordinates."
        )

    win_a = origin_entry.time_window
    win_b = destination_entry.time_window

    overlap = intervals_overlap(
        win_a.window_start_utc,
        win_a.window_end_utc,
        win_b.window_start_utc,
        win_b.window_end_utc,
    )
    if overlap is not None:
        raise ValueError(
            f"Cannot claim a definite movement sequence from '{origin_entry.event_id}' to "
            f"'{destination_entry.event_id}': temporal uncertainty windows overlap "
            f"[{overlap[0].isoformat()}, {overlap[1].isoformat()})."
        )

    if win_a.window_end_utc > win_b.window_start_utc:
        raise ValueError(
            f"Cannot build MovementSegment from '{origin_entry.event_id}' to "
            f"'{destination_entry.event_id}': origin window ends after destination window starts."
        )

    dist_km = haversine_distance_km(origin_coords, dest_coords)
    metrics = calculate_transit_metrics(
        win_a,
        win_b,
        dist_km,
        speed_threshold_kmh=speed_threshold_kmh,
    )

    intervening_ids: list[str] = (
        [str(eid) for eid in intervening_unlocated_event_ids]
        if intervening_unlocated_event_ids
        else []
    )
    has_gaps = len(intervening_ids) > 0
    continuity_note: Optional[str] = (
        f"Incomplete spatial continuity: {len(intervening_ids)} intervening event(s) "
        f"({', '.join(intervening_ids)}) lacked coordinates between "
        f"'{origin_entry.event_id}' and '{destination_entry.event_id}'. "
        "This segment represents net displacement between known locations, not an uninterrupted path."
        if has_gaps
        else None
    )

    segment_id = (
        f"seg-{clean_entity_id}-{origin_entry.event_id}-to-{destination_entry.event_id}"
    )

    return MovementSegment(
        segment_id=segment_id,
        case_id=origin_entry.case_id,
        entity_id=clean_entity_id,
        origin_event_id=origin_entry.event_id,
        destination_event_id=destination_entry.event_id,
        origin_source_evidence_id=origin_entry.source_evidence_id,
        destination_source_evidence_id=destination_entry.source_evidence_id,
        origin_location_name=origin_entry.location_name,
        destination_location_name=destination_entry.location_name,
        origin_coordinates=origin_coords,
        destination_coordinates=dest_coords,
        origin_time_window=win_a,
        destination_time_window=win_b,
        ordering_status=TemporalOrderingStatus.STRICTLY_ORDERED,
        distance_km=metrics.distance_km,
        nominal_elapsed_seconds=metrics.nominal_elapsed_seconds,
        min_possible_elapsed_seconds=metrics.min_possible_elapsed_seconds,
        max_possible_elapsed_seconds=metrics.max_possible_elapsed_seconds,
        min_required_speed_kmh=metrics.min_required_speed_kmh,
        nominal_speed_kmh=metrics.nominal_speed_kmh,
        configured_speed_threshold_kmh=metrics.configured_speed_threshold_kmh,
        exceeds_configured_speed_threshold=metrics.exceeds_configured_speed_threshold,
        threshold_evaluation=metrics.threshold_evaluation,
        threshold_assumption_note=metrics.threshold_assumption_note,
        intervening_unlocated_event_ids=intervening_ids,
        has_unlocated_intervening_events=has_gaps,
        continuity_note=continuity_note,
    )


def group_timeline_entries_by_entity(
    entries: Sequence[TimelineEntry],
) -> dict[str, list[TimelineEntry]]:
    """Group TimelineEntry records by involved_entity_ids in deterministic order.

    - Preserves events with missing coordinates alongside located events.
    - Deduplicates repeated entity IDs within the same event while preserving entry order.
    """
    grouped: dict[str, list[TimelineEntry]] = {}
    for entry in entries:
        seen_in_event: set[str] = set()
        for raw_entity_id in entry.involved_entity_ids:
            if not isinstance(raw_entity_id, str) or not raw_entity_id.strip():
                raise ValueError(
                    f"Event '{entry.event_id}' contains an empty or non-string entity ID."
                )
            entity_id = raw_entity_id.strip()
            if entity_id in seen_in_event:
                continue
            seen_in_event.add(entity_id)
            grouped.setdefault(entity_id, []).append(entry)

    return {eid: grouped[eid] for eid in sorted(grouped.keys())}


def reconstruct_spatial_trajectories(
    events: Sequence[Mapping[str, Any] | BaseModel | Any],
    evidence_by_id: Optional[Mapping[str, Mapping[str, Any] | BaseModel | Any]] = None,
    *,
    case_id: Optional[str] = None,
    speed_threshold_kmh: float = DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
    estimated_half_width_seconds: float = DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
) -> SpatialReconstructionResult:
    """Reconstruct per-entity timelines, strictly ordered movement segments, and uncertain spatial overlaps.

    Forensic guarantees:
    1. Never mutates input Event or Evidence objects.
    2. Validates all provided coordinates against WGS84 bounds before trajectory assembly,
       failing clearly with the offending event_id if malformed or out-of-range.
    3. Preserves unlocated events (coordinates=None) in each entity's timeline_entries and
       unlocated_event_ids without inventing coordinates.
    4. Generates directed MovementSegments ONLY when consecutive located events for an entity
       have non-overlapping half-open windows [start, end) and neither event is part of an
       unresolved temporal overlap with another located event of the same entity.
    5. Records overlapping located event pairs in uncertain_spatial_overlaps with
       ordering_status = CONCURRENT_UNCERTAIN.
    """
    if not isinstance(events, Sequence) or isinstance(events, (str, bytes)):
        raise ValueError("Invalid events input: expected a sequence of Event records.")

    threshold = _validate_speed_threshold(speed_threshold_kmh)
    half_width = _validate_estimated_half_width(estimated_half_width_seconds)

    # Validate coordinates on all input events upfront (without mutating inputs)
    validated_coords_by_event_id: dict[str, Optional[GeoPoint]] = {}
    for idx, event in enumerate(events):
        raw_event_id = extract_field(event, "event_id", f"<index_{idx}>")
        raw_coords = extract_field(event, "coordinates", None)
        try:
            validated_pt = validate_geopoint(raw_coords)
        except ValueError as exc:
            raise ValueError(
                f"Event '{raw_event_id}' coordinate validation failed: {exc}"
            ) from exc
        if isinstance(raw_event_id, str) and raw_event_id.strip():
            validated_coords_by_event_id[raw_event_id.strip()] = validated_pt

    timeline = reconstruct_timeline(
        events,
        evidence_by_id=evidence_by_id,
        case_id=case_id,
        estimated_half_width_seconds=half_width,
    )

    grouped_entries = group_timeline_entries_by_entity(timeline.entries)
    trajectories_by_entity: dict[str, EntityTrajectory] = {}

    for entity_id, entity_entries in grouped_entries.items():
        located_entries: list[TimelineEntry] = []
        located_points: dict[str, GeoPoint] = {}
        unlocated_event_ids: list[str] = []

        for entry in entity_entries:
            pt = validated_coords_by_event_id.get(entry.event_id)
            if pt is None and entry.coordinates is not None:
                pt = validate_geopoint(entry.coordinates)
            if pt is not None:
                located_entries.append(entry)
                located_points[entry.event_id] = pt
            else:
                unlocated_event_ids.append(entry.event_id)

        located_event_ids = [e.event_id for e in located_entries]

        # Identify all pairwise overlapping located events for this entity
        entity_overlapping_located_ids: set[str] = set()
        uncertain_overlaps: list[SpatialTemporalOverlap] = []

        n_loc = len(located_entries)
        for i in range(n_loc):
            entry_a = located_entries[i]
            win_a = entry_a.time_window
            pt_a = located_points[entry_a.event_id]

            for j in range(i + 1, n_loc):
                entry_b = located_entries[j]
                win_b = entry_b.time_window
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
                    pt_b = located_points[entry_b.event_id]
                    dist_km = haversine_distance_km(pt_a, pt_b)

                    sep_ab = (win_b.window_end_utc - win_a.window_start_utc).total_seconds()
                    sep_ba = (win_a.window_end_utc - win_b.window_start_utc).total_seconds()
                    max_sep = max(0.0, sep_ab, sep_ba)

                    entity_overlapping_located_ids.add(entry_a.event_id)
                    entity_overlapping_located_ids.add(entry_b.event_id)

                    uncertain_overlaps.append(
                        SpatialTemporalOverlap(
                            entity_id=entity_id,
                            event_a_id=entry_a.event_id,
                            event_b_id=entry_b.event_id,
                            source_evidence_a_id=entry_a.source_evidence_id,
                            source_evidence_b_id=entry_b.source_evidence_id,
                            location_a_name=entry_a.location_name,
                            location_b_name=entry_b.location_name,
                            coordinates_a=pt_a,
                            coordinates_b=pt_b,
                            time_window_a=win_a,
                            time_window_b=win_b,
                            overlap_start_utc=ov_start,
                            overlap_end_utc=ov_end,
                            overlap_seconds=ov_seconds,
                            distance_km=dist_km,
                            max_possible_separation_seconds=max_sep,
                            ordering_status=TemporalOrderingStatus.CONCURRENT_UNCERTAIN,
                        )
                    )

        # Build MovementSegments only between consecutive located events that have a
        # defensible, strictly ordered chronological relationship.
        entry_index_by_id: dict[str, int] = {
            e.event_id: idx for idx, e in enumerate(entity_entries)
        }
        segments: list[MovementSegment] = []
        for k in range(n_loc - 1):
            origin_entry = located_entries[k]
            dest_entry = located_entries[k + 1]

            # If either event is part of an unresolved temporal overlap among this entity's
            # located events, do not claim a definite sequential transition involving it.
            if (
                origin_entry.event_id in entity_overlapping_located_ids
                or dest_entry.event_id in entity_overlapping_located_ids
            ):
                continue

            if (
                origin_entry.time_window.window_end_utc
                <= dest_entry.time_window.window_start_utc
            ):
                origin_idx = entry_index_by_id[origin_entry.event_id]
                dest_idx = entry_index_by_id[dest_entry.event_id]
                intervening_unlocated_ids: list[str] = []
                for idx, candidate in enumerate(entity_entries):
                    if candidate.event_id in located_points:
                        continue
                    in_index_range = origin_idx < idx < dest_idx
                    in_time_span = (
                        candidate.time_window.window_end_utc
                        > origin_entry.time_window.window_start_utc
                        and candidate.time_window.window_start_utc
                        < dest_entry.time_window.window_end_utc
                    )
                    if in_index_range or in_time_span:
                        intervening_unlocated_ids.append(candidate.event_id)

                segment = create_movement_segment(
                    origin_entry,
                    dest_entry,
                    entity_id,
                    speed_threshold_kmh=threshold,
                    intervening_unlocated_event_ids=intervening_unlocated_ids,
                )
                segments.append(segment)

        total_distance = sum(seg.distance_km for seg in segments)

        trajectories_by_entity[entity_id] = EntityTrajectory(
            case_id=timeline.case_id,
            entity_id=entity_id,
            timeline_entries=entity_entries,
            located_event_ids=located_event_ids,
            unlocated_event_ids=unlocated_event_ids,
            segments=segments,
            uncertain_spatial_overlaps=uncertain_overlaps,
            total_strictly_ordered_distance_km=total_distance,
        )

    return SpatialReconstructionResult(
        case_id=timeline.case_id,
        timeline=timeline,
        trajectories_by_entity=trajectories_by_entity,
        configured_speed_threshold_kmh=threshold,
    )


def build_entity_trajectories(
    events: Sequence[Mapping[str, Any] | BaseModel | Any],
    evidence_by_id: Optional[Mapping[str, Mapping[str, Any] | BaseModel | Any]] = None,
    *,
    case_id: Optional[str] = None,
    speed_threshold_kmh: float = DEFAULT_REFERENCE_SPEED_THRESHOLD_KMH,
    estimated_half_width_seconds: float = DEFAULT_ESTIMATED_HALF_WIDTH_SECONDS,
) -> dict[str, EntityTrajectory]:
    """Convenience wrapper returning the deterministic entity_id -> EntityTrajectory mapping."""
    result = reconstruct_spatial_trajectories(
        events,
        evidence_by_id=evidence_by_id,
        case_id=case_id,
        speed_threshold_kmh=speed_threshold_kmh,
        estimated_half_width_seconds=estimated_half_width_seconds,
    )
    return result.trajectories_by_entity
