import type { CaseDetail, ProvenanceClass, RelationType } from '../types';

export type NodeCategory = 'person' | 'device' | 'vehicle' | 'location' | 'organization' | 'unclassified' | 'event' | 'evidence';

/** Edge kinds: canonical relation types plus two structural links derived from record fields. */
export type EdgeKind = RelationType | 'INVOLVES' | 'SOURCE_OF';

export interface GraphNode {
  id: string;
  label: string;
  sublabel: string;
  category: NodeCategory;
  x: number;
  y: number;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  kind: EdgeKind;
  label: string;
  provenance: ProvenanceClass | 'structural';
}

export const CATEGORY_LABEL: Record<NodeCategory, string> = {
  person: 'Person',
  device: 'Device',
  vehicle: 'Vehicle',
  location: 'Location',
  organization: 'Organization',
  unclassified: 'Account / unclassified',
  event: 'Event',
  evidence: 'Evidence artifact',
};

export const EDGE_KIND_LABEL: Record<EdgeKind, string> = {
  COMMUNICATED_WITH: 'Communicated with',
  PRESENT_AT: 'Present at',
  CO_OCCURRED_WITH: 'Co-occurred with',
  OWNS: 'Owns / holds',
  ASSOCIATE_OF: 'Associate of',
  CONTRADICTS: 'Contradicts',
  INVOLVES: 'Event involvement',
  SOURCE_OF: 'Evidence source',
};

/** Presentation layout hints (1000×640 canvas). Unknown ids fall back to a ring layout. */
const LAYOUT: Record<string, [number, number]> = {
  'ENT-PA': [500, 150],
  'ENT-DEV': [290, 250],
  'ENT-BADGE': [710, 250],
  'ENT-LOC': [500, 365],
  'ENT-VEH': [730, 455],
  'ENT-ACC': [250, 445],
  'EVT-1': [110, 540],
  'EVT-2': [250, 90],
  'EVT-3': [600, 560],
  'EVT-4': [890, 340],
  'EVT-5': [410, 540],
  'EV-01': [790, 595],
  'EV-02': [930, 200],
  'EV-03': [90, 70],
  'EV-04': [80, 620],
  'EV-05': [300, 610],
};

function categoryOf(entityType: string | null): NodeCategory {
  switch (entityType) {
    case 'PERSON':
      return 'person';
    case 'DEVICE':
    case 'PHONE_NUMBER':
      return 'device';
    case 'VEHICLE':
      return 'vehicle';
    case 'LOCATION':
      return 'location';
    case 'ORGANIZATION':
      return 'organization';
    default:
      return 'unclassified';
  }
}

export function buildGraph(detail: CaseDetail): { nodes: GraphNode[]; edges: GraphEdge[] } {
  const raw: Omit<GraphNode, 'x' | 'y'>[] = [
    ...detail.entities.map((e) => ({ id: e.entity_id, label: e.canonical_name, sublabel: e.entity_type ?? 'UNCLASSIFIED', category: categoryOf(e.entity_type) })),
    ...detail.events.map((e) => ({ id: e.event_id, label: e.title, sublabel: e.event_id, category: 'event' as const })),
    ...detail.evidence.map((e) => ({ id: e.evidence_id, label: e.evidence_id, sublabel: e.source, category: 'evidence' as const })),
  ];
  const nodes = raw.map((node, index) => {
    const hint = LAYOUT[node.id];
    const angle = (index / raw.length) * Math.PI * 2;
    return { ...node, x: hint?.[0] ?? 500 + Math.cos(angle) * 280, y: hint?.[1] ?? 320 + Math.sin(angle) * 240 };
  });

  const edges: GraphEdge[] = [
    ...detail.relationships.map((r) => ({
      id: r.relationship_id,
      source: r.source_id,
      target: r.target_id,
      kind: r.relation_type,
      label: r.label,
      provenance: r.provenance,
    })),
    ...detail.events.flatMap((event) =>
      event.involved_entity_ids.map((entityId) => ({
        id: `inv-${event.event_id}-${entityId}`,
        source: event.event_id,
        target: entityId,
        kind: 'INVOLVES' as const,
        label: 'involves',
        provenance: 'structural' as const,
      })),
    ),
    ...detail.events.map((event) => ({
      id: `src-${event.source_evidence_id}-${event.event_id}`,
      source: event.source_evidence_id,
      target: event.event_id,
      kind: 'SOURCE_OF' as const,
      label: 'source of',
      provenance: 'structural' as const,
    })),
  ];
  return { nodes, edges };
}

/** Breadth-first shortest path over the visible edges. Returns edge ids, or null when unreachable. */
export function shortestPath(edges: GraphEdge[], from: string, to: string): string[] | null {
  if (from === to) return [];
  const adjacency = new Map<string, { next: string; edge: string }[]>();
  for (const edge of edges) {
    adjacency.set(edge.source, [...(adjacency.get(edge.source) ?? []), { next: edge.target, edge: edge.id }]);
    adjacency.set(edge.target, [...(adjacency.get(edge.target) ?? []), { next: edge.source, edge: edge.id }]);
  }
  const previous = new Map<string, { node: string; edge: string }>();
  const queue = [from];
  const seen = new Set([from]);
  while (queue.length > 0) {
    const node = queue.shift()!;
    for (const { next, edge } of adjacency.get(node) ?? []) {
      if (seen.has(next)) continue;
      seen.add(next);
      previous.set(next, { node, edge });
      if (next === to) {
        const path: string[] = [];
        let cursor = to;
        while (cursor !== from) {
          const step = previous.get(cursor)!;
          path.unshift(step.edge);
          cursor = step.node;
        }
        return path;
      }
      queue.push(next);
    }
  }
  return null;
}
