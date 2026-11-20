import { requestJson, type RequestOptions } from './http';
import type {
  ReviewCategory,
  ReviewItem,
  TemporalOrderingStatus,
  TimePrecision,
  WorkspaceEvent,
} from '../features/workspace/types';

/** Standard UUID regex. */
const UUID_REGEX = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Normalizes case identifiers to compliant UUID strings. */
export function normalizeCaseId(caseId: string): string {
  const trimmed = caseId.trim();
  if (UUID_REGEX.test(trimmed)) {
    return trimmed.toLowerCase();
  }
  const numMatch = trimmed.match(/\d+/);
  if (numMatch) {
    const padded = numMatch[0].padStart(12, '0');
    return `00000000-0000-0000-0000-${padded}`;
  }
  return '00000000-0000-0000-0000-000000000001';
}

/* ----------------------------------------------------------------- DTO Types */

export interface GeoPointDto {
  lat: number;
  lon: number;
}

export interface NormalizedTimeWindowDto {
  nominal_utc: string;
  precision: TimePrecision;
  raw_precision_input: string;
  window_start_utc: string;
  window_end_utc: string;
  uncertainty_span_seconds: number;
  has_explicit_bounds: boolean;
  is_estimated_window_assumption: boolean;
  estimated_half_width_seconds?: number;
  duration_seconds?: number;
  source_timezone: string;
  clock_offset_seconds: number;
}

export interface TimelineEntryDto {
  event_id: string;
  source_evidence_id: string;
  time_window: NormalizedTimeWindowDto;
  coordinates?: GeoPointDto;
  raw_description?: string;
  entity_ids: string[];
  metadata: Record<string, unknown>;
  ordering_status: TemporalOrderingStatus;
  concurrent_event_ids: string[];
  /** Producer timestamp exactly as supplied (backend-rust ≥ 98a8bd8). */
  raw_timestamp?: string;
  /** Values the reconstruction used that were not observed (backend-rust ≥ 98a8bd8). */
  assumptions?: ReconstructionAssumptionDto[];
}

export interface ReconstructionAssumptionDto {
  kind: string;
  field: string;
  assumed_value: string;
  basis: string;
}

export interface TimelineOverlapPairDto {
  event_a_id: string;
  event_b_id: string;
  overlap_start_utc: string;
  overlap_end_utc: string;
  overlap_span_seconds: number;
}

export interface TimelineReconstructionResultDto {
  case_id?: string;
  total_events: number;
  entries: TimelineEntryDto[];
  overlaps: TimelineOverlapPairDto[];
  strictly_ordered_count: usizeNumber;
  concurrent_uncertain_count: usizeNumber;
  /** Candidates excluded with a reason (backend-rust ≥ 52953c3). */
  excluded_events?: { event_id: string; source_evidence_id: string; reason: string }[];
}

type usizeNumber = number;

export interface TransitTimeMetricsDto {
  distance_km: number;
  nominal_elapsed_seconds: number;
  min_possible_elapsed_seconds: number;
  max_possible_elapsed_seconds: number;
  min_required_speed_kmh?: number;
  nominal_speed_kmh?: number;
  configured_speed_threshold_kmh: number;
  exceeds_configured_speed_threshold: boolean;
  threshold_evaluation: string;
}

export interface MovementSegmentDto {
  origin_event_id: string;
  destination_event_id: string;
  entity_id: string;
  origin_coordinates: GeoPointDto;
  destination_coordinates: GeoPointDto;
  transit_metrics: TransitTimeMetricsDto;
  has_intervening_unlocated_events: boolean;
  intervening_unlocated_event_ids: string[];
}

export interface SpatialTemporalOverlapDto {
  entity_id: string;
  event_a_id: string;
  event_b_id: string;
  overlap_start_utc: string;
  overlap_end_utc: string;
  distance_km: number;
  is_co_located: boolean;
}

export interface EntityTrajectoryDto {
  entity_id: string;
  waypoints: TimelineEntryDto[];
  segments: MovementSegmentDto[];
  total_distance_km: number;
  max_min_required_speed_kmh?: number;
}

export interface SpatialReconstructionResultDto {
  case_id?: string;
  trajectories: EntityTrajectoryDto[];
  overlaps: SpatialTemporalOverlapDto[];
}

export interface ObservedLeadBasisDto {
  origin_event_id: string;
  destination_event_id: string;
  origin_evidence_id: string;
  destination_evidence_id: string;
  origin_timestamp_raw: string;
  destination_timestamp_raw: string;
  origin_coordinates?: GeoPointDto;
  destination_coordinates?: GeoPointDto;
  observed_distance_km?: number;
}

export interface DerivedLeadInferenceDto {
  post_activity_min_required_speed_kmh?: number;
  reference_speed_threshold_kmh: number;
  max_possible_elapsed_seconds: number;
  min_possible_elapsed_seconds: number;
  nominal_elapsed_seconds: number;
}

export interface ContradictionLeadDto {
  lead_id: string;
  lead_type: 'SPATIAL_TRANSIT_IMPOSSIBLE' | 'TEMPORAL_ACTIVITY_INFEASIBLE';
  entity_id: string;
  event_a_id: string;
  event_b_id: string;
  observed_basis: ObservedLeadBasisDto;
  derived_inference: DerivedLeadInferenceDto;
  neutral_explanation: string;
  limitations: string[];
  generated_at: string;
}

export interface ContradictionAnalysisResultDto {
  case_id?: string;
  leads: ContradictionLeadDto[];
  total_evaluated_segments: number;
  flagged_segments_count: number;
  disclaimer: string;
}

export interface FullCaseReconstructionResultDto {
  case_id?: string;
  timeline: TimelineReconstructionResultDto;
  spatial: SpatialReconstructionResultDto;
  contradictions: ContradictionAnalysisResultDto;
}

export interface ReconstructionOptionsDto {
  default_timezone?: string;
  default_clock_offset_seconds?: number;
  speed_threshold_kmh?: number;
  estimated_half_width_seconds?: number;
  case_id?: string;
}

/* ----------------------------------------------------------------- Runtime Parsers */

export function parseFullReconstruction(data: unknown): FullCaseReconstructionResultDto {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('Reconstruction response must be an object.');
  }
  return data as FullCaseReconstructionResultDto;
}

export function parseTimelineResult(data: unknown): TimelineReconstructionResultDto {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('Timeline response must be an object.');
  }
  return data as TimelineReconstructionResultDto;
}

export function parseSpatialResult(data: unknown): SpatialReconstructionResultDto {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('Spatial response must be an object.');
  }
  return data as SpatialReconstructionResultDto;
}

export function parseContradictionsResult(data: unknown): ContradictionAnalysisResultDto {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('Contradictions response must be an object.');
  }
  return data as ContradictionAnalysisResultDto;
}

/* ----------------------------------------------------------------- API Functions */

/**
 * Triggers a full forensic reconstruction for a case on the Rust backend.
 */
export async function triggerCaseReconstruction(
  caseId: string,
  options?: ReconstructionOptionsDto,
  requestOptions: RequestOptions = {},
): Promise<FullCaseReconstructionResultDto> {
  const normCaseId = normalizeCaseId(caseId);
  return requestJson<FullCaseReconstructionResultDto>(
    `/api/v1/cases/${encodeURIComponent(normCaseId)}/reconstruction`,
    parseFullReconstruction,
    {
      ...requestOptions,
      init: {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ options: options ?? null }),
        ...requestOptions.init,
      },
    },
  );
}

/**
 * Retrieves the latest reconstruction result for a case from the Rust backend.
 */
export async function getCaseReconstruction(
  caseId: string,
  requestOptions: RequestOptions = {},
): Promise<FullCaseReconstructionResultDto> {
  const normCaseId = normalizeCaseId(caseId);
  return requestJson<FullCaseReconstructionResultDto>(
    `/api/v1/cases/${encodeURIComponent(normCaseId)}/reconstruction`,
    parseFullReconstruction,
    requestOptions,
  );
}

/**
 * Retrieves timeline reconstruction results for a case.
 */
export async function getCaseTimeline(
  caseId: string,
  requestOptions: RequestOptions = {},
): Promise<TimelineReconstructionResultDto> {
  const normCaseId = normalizeCaseId(caseId);
  return requestJson<TimelineReconstructionResultDto>(
    `/api/v1/cases/${encodeURIComponent(normCaseId)}/timeline`,
    parseTimelineResult,
    requestOptions,
  );
}

/**
 * Retrieves spatial reconstruction results (trajectories and movement segments) for a case.
 */
export async function getCaseSpatial(
  caseId: string,
  requestOptions: RequestOptions = {},
): Promise<SpatialReconstructionResultDto> {
  const normCaseId = normalizeCaseId(caseId);
  return requestJson<SpatialReconstructionResultDto>(
    `/api/v1/cases/${encodeURIComponent(normCaseId)}/spatial`,
    parseSpatialResult,
    requestOptions,
  );
}

/**
 * Retrieves contradiction analysis leads for a case.
 */
export async function getCaseContradictions(
  caseId: string,
  requestOptions: RequestOptions = {},
): Promise<ContradictionAnalysisResultDto> {
  const normCaseId = normalizeCaseId(caseId);
  return requestJson<ContradictionAnalysisResultDto>(
    `/api/v1/cases/${encodeURIComponent(normCaseId)}/contradictions`,
    parseContradictionsResult,
    requestOptions,
  );
}

/* ----------------------------------------------------------------- View Model Adapters */

/**
 * Adapts backend `TimelineEntryDto` items into frontend `WorkspaceEvent` objects.
 */
export function adaptTimelineEntriesToWorkspaceEvents(
  entries: TimelineEntryDto[],
): WorkspaceEvent[] {
  return entries.map((entry) => {
    const rawMeta = entry.metadata || {};
    const eventType =
      typeof rawMeta.event_type === 'string'
        ? rawMeta.event_type
        : typeof rawMeta.call_type === 'string'
        ? `TELEPHONY_${rawMeta.call_type}`
        : 'OBSERVED_EVENT';

    const sourceRecordedTime =
      typeof entry.raw_timestamp === 'string' && entry.raw_timestamp
        ? entry.raw_timestamp
        : typeof rawMeta.call_timestamp === 'string'
        ? rawMeta.call_timestamp
        : typeof rawMeta.timestamp === 'string'
        ? rawMeta.timestamp
        : entry.time_window.nominal_utc;

    const isSyntheticCentroid = Boolean(rawMeta.location_synthetic_centroid);
    const cellTowerId =
      typeof rawMeta.location_cell_tower_id === 'string'
        ? rawMeta.location_cell_tower_id
        : undefined;

    const locationName = isSyntheticCentroid
      ? `${cellTowerId || entry.raw_description || 'Cell Tower'} [SYNTHETIC CENTROID — NOT GPS OBSERVATION]`
      : entry.raw_description;

    return {
      event_id: entry.event_id,
      title: entry.raw_description || `${eventType} (${entry.event_id})`,
      event_type: eventType,
      timestamp: entry.time_window.nominal_utc,
      timestamp_precision: entry.time_window.precision,
      source_recorded_time: sourceRecordedTime,
      source_timezone: entry.time_window.source_timezone,
      uncertainty: {
        window_start_utc: entry.time_window.window_start_utc,
        window_end_utc: entry.time_window.window_end_utc,
        basis: entry.time_window.has_explicit_bounds
          ? 'Explicit source bounds'
          : `Precision window (${entry.time_window.precision})`,
      },
      ordering_status: entry.ordering_status,
      location_name: locationName,
      involved_entity_ids: entry.entity_ids,
      source_evidence_id: entry.source_evidence_id,
      provenance: 'observed',
      clock_offset_seconds: entry.time_window.clock_offset_seconds,
      assumptions: entry.assumptions ?? [],
      frame: adaptFrameProvenance(rawMeta),
    };
  });
}

/** Frame provenance written by the backend video handoff into candidate metadata. */
function adaptFrameProvenance(meta: Record<string, unknown>): WorkspaceEvent['frame'] {
  const pts = meta.pts_seconds;
  const sha = meta.sha256_hash;
  if (typeof pts !== 'number' || typeof sha !== 'string') return undefined;
  const str = (key: string) => (typeof meta[key] === 'string' ? (meta[key] as string) : undefined);
  return {
    pts_seconds: pts,
    frame_index: typeof meta.frame_index === 'number' ? (meta.frame_index as number) : undefined,
    sha256: sha,
    artifact_reference: str('frame_artifact_reference'),
    relative_path: str('relative_path'),
    source_video_sha256: str('source_video_sha256'),
    anchor_basis: str('utc_anchor_basis'),
  };
}

/**
 * Adapts backend `ContradictionLeadDto` items into frontend `ReviewItem` objects.
 */
export function adaptContradictionLeadsToReviewItems(
  leads: ContradictionLeadDto[],
): ReviewItem[] {
  return leads.map((lead) => {
    const isSpatial = lead.lead_type === 'SPATIAL_TRANSIT_IMPOSSIBLE';
    const category: ReviewCategory = 'conflicting';

    const speedDesc = lead.derived_inference.post_activity_min_required_speed_kmh
      ? `${lead.derived_inference.post_activity_min_required_speed_kmh.toFixed(1)} km/h`
      : 'Infinite / zero transit time';

    const discrepancy = isSpatial
      ? `Transit speed required (${speedDesc}) exceeds reference threshold (${lead.derived_inference.reference_speed_threshold_kmh} km/h)`
      : `Temporal activity infeasible across half-open uncertainty intervals`;

    return {
      id: lead.lead_id,
      category,
      title: isSpatial
        ? `Physical transit discrepancy · ${lead.entity_id}`
        : `Activity duration infeasible · ${lead.entity_id}`,
      summary: lead.neutral_explanation,
      records: [
        {
          evidence_id: lead.observed_basis.origin_evidence_id,
          label: 'Departure event',
          value: lead.observed_basis.origin_timestamp_raw,
        },
        {
          evidence_id: lead.observed_basis.destination_evidence_id,
          label: 'Arrival event',
          value: lead.observed_basis.destination_timestamp_raw,
        },
      ],
      discrepancy,
      related_event_ids: [lead.event_a_id, lead.event_b_id],
      related_entity_ids: [lead.entity_id],
      status: 'flagged',
      suggested_action:
        'Verify camera clock calibration offsets, carrier cell tower centroids, and physical transit records.',
      limitations: lead.limitations,
    };
  });
}
