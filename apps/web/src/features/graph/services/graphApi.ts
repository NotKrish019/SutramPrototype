import { requestJson, type RequestOptions } from '../../../services/http';
import type {
  GraphNeighborResponseDto,
  GraphPathResponseDto,
  GraphSummaryDto,
} from '../types/graph';

/** Standard UUID regex. */
const UUID_REGEX = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Normalizes case slugs (e.g., `CASE-026`) to compliant UUIDs if needed.
 */
export function normalizeCaseId(caseId: string): string {
  const trimmed = caseId.trim();
  if (UUID_REGEX.test(trimmed)) {
    return trimmed.toLowerCase();
  }

  // Handle synthetic case slug like CASE-026 -> 00000000-0000-0000-0000-000000000026
  const numMatch = trimmed.match(/\d+/);
  if (numMatch) {
    const padded = numMatch[0].padStart(12, '0');
    return `00000000-0000-0000-0000-${padded}`;
  }

  // Fallback constant UUID
  return '00000000-0000-0000-0000-000000000001';
}

/** Runtime parser for GraphSummaryDto */
export function parseGraphSummary(data: unknown): GraphSummaryDto {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('GraphSummary response must be an object.');
  }
  const obj = data as Record<string, unknown>;
  return {
    case_id: String(obj.case_id || ''),
    evidence_count: Number(obj.evidence_count || 0),
    extraction_run_count: Number(obj.extraction_run_count || 0),
    entity_candidate_count: Number(obj.entity_candidate_count || 0),
    event_candidate_count: Number(obj.event_candidate_count || 0),
    relationship_assertion_count: Number(obj.relationship_assertion_count || 0),
    reconstruction_run_count: Number(obj.reconstruction_run_count || 0),
    contradiction_lead_count: Number(obj.contradiction_lead_count || 0),
    graph_synchronization_status: String(obj.graph_synchronization_status || 'UNKNOWN'),
    last_projection_time: obj.last_projection_time ? String(obj.last_projection_time) : null,
    identifier_privacy: String(obj.identifier_privacy || 'unconfigured'),
  };
}

/** Runtime parser for GraphNeighborResponseDto */
export function parseGraphNeighbors(data: unknown): GraphNeighborResponseDto {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('GraphNeighbor response must be an object.');
  }
  const obj = data as Record<string, unknown>;
  return {
    case_id: String(obj.case_id || ''),
    root_entity_id: String(obj.root_entity_id || ''),
    depth: Number(obj.depth || 1),
    nodes: Array.isArray(obj.nodes) ? (obj.nodes as GraphNeighborResponseDto['nodes']) : [],
    relationships: Array.isArray(obj.relationships)
      ? (obj.relationships as GraphNeighborResponseDto['relationships'])
      : [],
  };
}

/** Runtime parser for GraphPathResponseDto */
export function parseGraphPath(data: unknown): GraphPathResponseDto {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('GraphPath response must be an object.');
  }
  const obj = data as Record<string, unknown>;
  return {
    case_id: String(obj.case_id || ''),
    from_entity_id: String(obj.from_entity_id || ''),
    to_entity_id: String(obj.to_entity_id || ''),
    found: Boolean(obj.found),
    nodes: Array.isArray(obj.nodes) ? (obj.nodes as GraphPathResponseDto['nodes']) : [],
    relationships: Array.isArray(obj.relationships)
      ? (obj.relationships as GraphPathResponseDto['relationships'])
      : [],
  };
}

/**
 * Fetches the case knowledge graph summary from the backend.
 * `GET /api/v1/cases/:case_id/graph/summary`
 */
export async function fetchGraphSummary(
  caseId: string,
  options: Pick<RequestOptions, 'signal' | 'timeoutMs'> = {},
): Promise<GraphSummaryDto> {
  const normId = normalizeCaseId(caseId);
  const path = `/api/v1/cases/${encodeURIComponent(normId)}/graph/summary`;
  return requestJson(path, parseGraphSummary, options);
}

/**
 * Fetches neighbors of an entity candidate within the case boundary.
 * `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors?depth=1&limit=50`
 */
export async function fetchEntityNeighbors(
  caseId: string,
  entityId: string,
  depth = 1,
  limit = 50,
  options: Pick<RequestOptions, 'signal' | 'timeoutMs'> = {},
): Promise<GraphNeighborResponseDto> {
  const normId = normalizeCaseId(caseId);
  const safeDepth = Math.max(1, Math.min(depth, 3));
  const safeLimit = Math.max(1, Math.min(limit, 100));
  const path = `/api/v1/cases/${encodeURIComponent(normId)}/graph/entities/${encodeURIComponent(
    entityId,
  )}/neighbors?depth=${safeDepth}&limit=${safeLimit}`;
  return requestJson(path, parseGraphNeighbors, options);
}

/**
 * Finds the shortest evidentiary path between two entities in a case.
 * `GET /api/v1/cases/:case_id/graph/path?from=...&to=...&max_hops=3`
 */
export async function fetchEntityPath(
  caseId: string,
  fromEntityId: string,
  toEntityId: string,
  maxHops = 3,
  options: Pick<RequestOptions, 'signal' | 'timeoutMs'> = {},
): Promise<GraphPathResponseDto> {
  const normId = normalizeCaseId(caseId);
  const safeHops = Math.max(1, Math.min(maxHops, 5));
  const path = `/api/v1/cases/${encodeURIComponent(normId)}/graph/path?from=${encodeURIComponent(
    fromEntityId,
  )}&to=${encodeURIComponent(toEntityId)}&max_hops=${safeHops}`;
  return requestJson(path, parseGraphPath, options);
}
