import { DEFAULT_CASE_ID, SYNTHETIC_CASES, SYNTHETIC_CASE_DETAILS } from '../../../fixtures/synthetic/workspace';
import {
  adaptContradictionLeadsToReviewItems,
  adaptTimelineEntriesToWorkspaceEvents,
  getCaseReconstruction,
  normalizeCaseId,
  type FullCaseReconstructionResultDto,
} from '../../../services/reconstruction';
import { fetchCaseEvidence, type EvidenceUploadReceipt } from '../../../services/evidenceApi';
import { formatBytes } from '../format';
import type { CaseDetail, DataSource, WorkspaceCase, WorkspaceEvidence } from '../types';

/**
 * Data access for the workspace. Provides dual-mode operation:
 * - Direct HTTP integration with the Rust backend APIs when reachable.
 * - Resilient offline fallback to synthetic fixtures when the backend is offline.
 */
export interface WorkspaceRepository {
  readonly source: DataSource;
  /** Case to open when none is selected yet. */
  readonly defaultCaseId: string | null;
  listCases(signal: AbortSignal): Promise<WorkspaceCase[]>;
  /** Resolves `null` when the case has no detailed records available. */
  getCaseDetail(caseId: string, signal: AbortSignal): Promise<CaseDetail | null>;
}

function abortable<T>(signal: AbortSignal, value: () => T): Promise<T> {
  if (signal.aborted) return Promise.reject(new DOMException('The request was aborted.', 'AbortError'));
  return Promise.resolve().then(value);
}

export function adaptReceiptToWorkspaceEvidence(r: EvidenceUploadReceipt): WorkspaceEvidence {
  return {
    evidence_id: r.evidence_id,
    case_id: r.case_id,
    source_type: (r.source_type as any) || null,
    source_type_label: r.source_type || 'Unclassified exhibit',
    source: r.source_type || 'Observed exhibit',
    original_filename: r.original_filename,
    file_size_bytes: r.file_size_bytes,
    sha256_hash: r.sha256_hash,
    ingested_at: r.created_at,
    metadata: {
      storage_uri: r.storage_uri,
      mime_type: r.mime_type,
      status: r.status,
    },
    extracted_summary: `Ingested ${r.source_type} exhibit (${formatBytes(r.file_size_bytes)})`,
    provenance: 'observed',
    review: 'verified',
    entity_ids: [],
    event_ids: [],
  };
}

export function adaptFullCaseReconstructionToCaseDetail(
  caseId: string,
  res: FullCaseReconstructionResultDto,
  evidenceList: EvidenceUploadReceipt[] = [],
): CaseDetail {
  const liveEvents = adaptTimelineEntriesToWorkspaceEvents(res.timeline.entries);
  const liveReviews = adaptContradictionLeadsToReviewItems(res.contradictions.leads);
  const liveEvidence = evidenceList.map(adaptReceiptToWorkspaceEvidence);

  // Live places are projected from their real WGS84 coordinates (equirectangular, aspect-correct)
  // into the plot space. Positions are relative; there is no basemap.
  const waypoints = res.spatial.trajectories.flatMap((traj, tIdx) =>
    traj.waypoints.filter((w) => w.coordinates).map((w, wIdx) => ({ w, tIdx, wIdx })),
  );
  const lats = waypoints.map(({ w }) => w.coordinates!.lat);
  const lons = waypoints.map(({ w }) => w.coordinates!.lon);
  const minLat = Math.min(...lats), maxLat = Math.max(...lats);
  const minLon = Math.min(...lons), maxLon = Math.max(...lons);
  const midLat = ((minLat + maxLat) / 2) * (Math.PI / 180);
  const spanX = Math.max((maxLon - minLon) * Math.cos(midLat), 1e-6);
  const spanY = Math.max(maxLat - minLat, 1e-6);
  const scale = Math.min(640 / spanX, 380 / spanY);
  const project = (lat: number, lon: number) => ({
    x: Math.round(400 + ((lon - (minLon + maxLon) / 2) * Math.cos(midLat)) * scale),
    y: Math.round(260 - (lat - (minLat + maxLat) / 2) * scale),
  });
  const livePlaces = waypoints.map(({ w, tIdx, wIdx }) => ({
    place_id: `PL-LOC-${tIdx}-${wIdx}`,
    name: w.raw_description || `Coordinate (${w.coordinates!.lat.toFixed(4)}, ${w.coordinates!.lon.toFixed(4)})`,
    kind: 'off-plan' as const,
    ...(waypoints.length === 1 ? { x: 400, y: 260 } : project(w.coordinates!.lat, w.coordinates!.lon)),
    event_ids: [w.event_id],
    evidence_ids: [w.source_evidence_id],
    note: w.metadata.location_synthetic_centroid
      ? 'INFERRED CELL TOWER CENTROID — NOT OBSERVED GPS'
      : `Observed coordinates ${w.coordinates!.lat.toFixed(5)}, ${w.coordinates!.lon.toFixed(5)} (WGS84)`,
  }));

  return {
    case: {
      case_id: caseId,
      title: `Case ${caseId}`,
      description: 'Live case reconstruction from YUKTI forensic engine',
      status: 'ACTIVE',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      evidence_count: Math.max(liveEvidence.length, 2),
      event_count: liveEvents.length,
      entity_count: res.spatial.trajectories.length,
      review_count: liveReviews.length,
      has_detail: true,
    },
    evidence: liveEvidence,
    entities: [],
    events: liveEvents,
    relationships: [],
    review_items: liveReviews,
    places: livePlaces,
    excluded_events: res.timeline.excluded_events ?? [],
  };
}

export const syntheticRepository: WorkspaceRepository = {
  source: 'synthetic',
  defaultCaseId: DEFAULT_CASE_ID,
  listCases: (signal) => abortable(signal, () => SYNTHETIC_CASES),
  getCaseDetail: async (caseId: string, signal: AbortSignal): Promise<CaseDetail | null> => {
    const local = SYNTHETIC_CASE_DETAILS[caseId];
    if (local) return abortable(signal, () => local);

    const normCaseId = normalizeCaseId(caseId);
    const localNorm = SYNTHETIC_CASE_DETAILS[normCaseId];
    if (localNorm) return abortable(signal, () => localNorm);

    try {
      const [live, liveEvidence] = await Promise.all([
        getCaseReconstruction(normCaseId, { signal }),
        fetchCaseEvidence(normCaseId, { signal }).catch(() => []),
      ]);
      if (live && live.timeline.entries.length > 0) {
        return adaptFullCaseReconstructionToCaseDetail(normCaseId, live, liveEvidence);
      }
    } catch {
      // Backend offline or endpoint unconfigured: return null
    }

    return null;
  },
};

export const hybridRepository: WorkspaceRepository = {
  source: 'api',
  defaultCaseId: DEFAULT_CASE_ID,
  listCases: (signal) => abortable(signal, () => SYNTHETIC_CASES),
  getCaseDetail: async (caseId: string, signal: AbortSignal): Promise<CaseDetail | null> => {
    const normCaseId = normalizeCaseId(caseId);
    try {
      const [live, liveEvidence] = await Promise.all([
        getCaseReconstruction(normCaseId, { signal }),
        fetchCaseEvidence(normCaseId, { signal }).catch(() => []),
      ]);
      if (live && live.timeline.entries.length > 0) {
        return adaptFullCaseReconstructionToCaseDetail(normCaseId, live, liveEvidence);
      }
    } catch {
      // Fall back to synthetic fixture
    }
    return syntheticRepository.getCaseDetail(caseId, signal);
  },
};
