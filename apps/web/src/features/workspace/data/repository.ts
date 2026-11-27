import { DEFAULT_CASE_ID, SYNTHETIC_CASES, SYNTHETIC_CASE_DETAILS } from '../../../fixtures/synthetic/workspace';
import {
  adaptContradictionLeadsToReviewItems,
  adaptTimelineEntriesToWorkspaceEvents,
  getCaseReconstruction,
  normalizeCaseId,
  type FullCaseReconstructionResultDto,
} from '../../../services/reconstruction';
import { fetchCaseEvidence, type EvidenceUploadReceipt } from '../../../services/evidenceApi';
import { fetchCase, fetchCases, type CaseRecordDto } from '../../../services/casesApi';
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
  record?: CaseRecordDto,
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
      title: record?.title ?? `Case ${caseId}`,
      fir_number: record?.fir_number ?? undefined,
      description: record?.description ?? 'Live case reconstruction from YUKTI forensic engine',
      status: record?.status ?? 'ACTIVE',
      created_at: record?.created_at ?? new Date().toISOString(),
      updated_at: record?.updated_at ?? new Date().toISOString(),
      source: 'api',
      evidence_count: liveEvidence.length,
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

/* ------------------------------------------------------------------ Live repository */

const SYNTHETIC_TAGGED: WorkspaceCase[] = SYNTHETIC_CASES.map((c) => ({ ...c, source: 'synthetic' }));

export function adaptCaseRecord(record: CaseRecordDto): WorkspaceCase {
  return {
    case_id: record.case_id,
    title: record.title,
    fir_number: record.fir_number ?? undefined,
    description: record.description ?? '',
    status: record.status,
    created_at: record.created_at,
    updated_at: record.updated_at,
    evidence_count: 0,
    event_count: 0,
    entity_count: 0,
    review_count: 0,
    has_detail: true,
    source: 'api',
  };
}

function emptyLiveDetail(record: CaseRecordDto, evidence: EvidenceUploadReceipt[]): CaseDetail {
  return {
    case: { ...adaptCaseRecord(record), evidence_count: evidence.length },
    evidence: evidence.map(adaptReceiptToWorkspaceEvidence),
    entities: [],
    events: [],
    relationships: [],
    review_items: [],
    places: [],
  };
}

/**
 * Used when the Rust API answers. Lists every case on the API (including those opened from the
 * `yukti` CLI) ahead of the synthetic demonstration cases, which stay available and labelled.
 */
export const liveRepository: WorkspaceRepository = {
  source: 'api',
  defaultCaseId: DEFAULT_CASE_ID,
  listCases: async (signal) => {
    const records = await fetchCases({ signal, timeoutMs: 5000 });
    const live = records
      .map(adaptCaseRecord)
      .sort((a, b) => b.updated_at.localeCompare(a.updated_at));
    return [...live, ...SYNTHETIC_TAGGED];
  },
  getCaseDetail: async (caseId, signal) => {
    const fixture = SYNTHETIC_CASE_DETAILS[caseId] ?? SYNTHETIC_CASE_DETAILS[normalizeCaseId(caseId)];
    if (fixture) return { ...fixture, case: { ...fixture.case, source: 'synthetic' } };

    const id = normalizeCaseId(caseId);
    const record = await fetchCase(id, { signal }).catch((error: unknown) => {
      if (signal.aborted) throw error;
      return null;
    });
    if (!record) return null;
    const [reconstruction, evidence] = await Promise.all([
      getCaseReconstruction(id, { signal, timeoutMs: 20_000 }).catch((error: unknown) => {
        if (signal.aborted) throw error;
        return null;
      }),
      fetchCaseEvidence(id, { signal }).catch(() => [] as EvidenceUploadReceipt[]),
    ]);
    if (!reconstruction || reconstruction.timeline.entries.length === 0) return emptyLiveDetail(record, evidence);
    return adaptFullCaseReconstructionToCaseDetail(id, reconstruction, evidence, record);
  },
};
