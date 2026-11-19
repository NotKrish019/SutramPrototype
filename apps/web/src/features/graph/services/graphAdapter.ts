import type { CaseDetail } from '../../workspace/types';
import type {
  BackendGraphNode,
  BackendGraphRelationship,
  CytoscapeEdgeData,
  CytoscapeElement,
  CytoscapeNodeData,
  ForensicNodeCategory,
  ProvenanceKind,
} from '../types/graph';

/**
 * Normalizes backend node category label to supported `ForensicNodeCategory`.
 */
export function normalizeCategory(label: string): ForensicNodeCategory {
  switch (label) {
    case 'Case':
      return 'Case';
    case 'Evidence':
      return 'Evidence';
    case 'ExtractionRun':
      return 'ExtractionRun';
    case 'EntityCandidate':
      return 'EntityCandidate';
    case 'EventCandidate':
      return 'EventCandidate';
    case 'RelationshipAssertion':
      return 'RelationshipAssertion';
    case 'ReconstructionRun':
      return 'ReconstructionRun';
    case 'ContradictionLead':
      return 'ContradictionLead';
    case 'TemporalCluster':
      return 'TemporalCluster';
    case 'SpatialLocus':
      return 'SpatialLocus';
    default:
      return 'EntityCandidate';
  }
}

/**
 * Derives a privacy-conscious display label.
 * Never displays raw phone numbers, IMEI, or IMSI if redacted/hashed.
 */
export function deriveSafeNodeLabel(node: BackendGraphNode): {
  label: string;
  sublabel: string;
  isRedacted: boolean;
} {
  const p = node.properties || {};

  if (typeof p.canonical_name === 'string' && p.canonical_name.trim().length > 0) {
    return {
      label: p.canonical_name,
      sublabel: (p.entity_type as string) || node.label,
      isRedacted: false,
    };
  }

  if (typeof p.title === 'string' && p.title.trim().length > 0) {
    return {
      label: p.title,
      sublabel: (p.event_type as string) || (p.conflict_type as string) || node.label,
      isRedacted: false,
    };
  }

  if (typeof p.fingerprint === 'string' && p.fingerprint.trim().length > 0) {
    const fp = p.fingerprint.trim();
    const shortFp = fp.length > 12 ? `${fp.slice(0, 6)}…${fp.slice(-4)}` : fp;
    return {
      label: `[HMAC] ${shortFp}`,
      sublabel: `${p.entity_type || 'ID'}:PROTECTED`,
      isRedacted: true,
    };
  }

  if (typeof p.sha256_hash === 'string' && p.sha256_hash.trim().length > 0) {
    const hash = p.sha256_hash.trim();
    return {
      label: `${hash.slice(0, 8)}…`,
      sublabel: (p.source_type as string) || 'FILE',
      isRedacted: false,
    };
  }

  return {
    label: node.id,
    sublabel: node.label,
    isRedacted: false,
  };
}

/**
 * Transforms backend graph API nodes into Cytoscape element definitions.
 */
export function adaptBackendNodes(
  backendNodes: BackendGraphNode[],
  rootEntityId?: string,
): CytoscapeElement[] {
  const seenIds = new Set<string>();
  const elements: CytoscapeElement[] = [];

  for (const node of backendNodes) {
    if (!node.id || seenIds.has(node.id)) continue;
    seenIds.add(node.id);

    const category = normalizeCategory(node.label);
    const { label, sublabel, isRedacted } = deriveSafeNodeLabel(node);
    const p = node.properties || {};

    const nodeData: CytoscapeNodeData = {
      id: node.id,
      label,
      sublabel,
      category,
      isRoot: node.id === rootEntityId,
      isRedacted,
      confidence: typeof p.confidence === 'number' ? p.confidence : null,
      rawProperties: p,
    };

    elements.push({
      group: 'nodes',
      data: nodeData,
    });
  }

  return elements;
}

/**
 * Transforms backend graph API relationships into Cytoscape element definitions.
 */
export function adaptBackendRelationships(
  backendRels: BackendGraphRelationship[],
): CytoscapeElement[] {
  const seenIds = new Set<string>();
  const elements: CytoscapeElement[] = [];

  for (const rel of backendRels) {
    if (!rel.id || seenIds.has(rel.id)) continue;
    seenIds.add(rel.id);

    const p = rel.properties || {};
    const relType = rel.relation_type || 'CONNECTED_TO';
    const isObserved = p.is_observed !== false;
    const isContradiction =
      relType === 'CONTRADICTS_EVENT' || relType === 'CONTRADICTS' || relType.includes('CONFLICT');

    let provenance: ProvenanceKind = 'observed';
    if (!isObserved) provenance = 'inferred';
    if (relType.startsWith('EXTRACTED_') || relType.startsWith('PRODUCED_')) {
      provenance = 'structural';
    }

    const edgeData: CytoscapeEdgeData = {
      id: rel.id,
      source: rel.source,
      target: rel.target,
      label: relType.toLowerCase().replace(/_/g, ' '),
      relationType: relType,
      provenance,
      isObserved,
      isContradiction,
      confidence: typeof p.confidence === 'number' ? p.confidence : null,
      rawProperties: p,
    };

    elements.push({
      group: 'edges',
      data: edgeData,
    });
  }

  return elements;
}

/**
 * Adapts local CaseDetail (synthetic scenario or fallback) into Cytoscape elements.
 */
export function adaptCaseDetailToCytoscape(detail: CaseDetail): CytoscapeElement[] {
  const elements: CytoscapeElement[] = [];
  const seenIds = new Set<string>();

  // 1. Entities
  for (const ent of detail.entities) {
    if (seenIds.has(ent.entity_id)) continue;
    seenIds.add(ent.entity_id);

    elements.push({
      group: 'nodes',
      data: {
        id: ent.entity_id,
        label: ent.canonical_name,
        sublabel: ent.entity_type || 'ENTITY',
        category: 'EntityCandidate',
        confidence: ent.confidence,
        rawProperties: {
          aliases: ent.aliases,
          source_evidence_ids: ent.source_evidence_ids,
          attributes: ent.attributes,
        },
      },
    });
  }

  // 2. Events
  for (const ev of detail.events) {
    if (seenIds.has(ev.event_id)) continue;
    seenIds.add(ev.event_id);

    elements.push({
      group: 'nodes',
      data: {
        id: ev.event_id,
        label: ev.title,
        sublabel: ev.event_type,
        category: 'EventCandidate',
        confidence: null,
        rawProperties: {
          timestamp: ev.timestamp,
          precision: ev.timestamp_precision,
          recorded_time: ev.source_recorded_time,
          ordering: ev.ordering_status,
          source_evidence_id: ev.source_evidence_id,
        },
      },
    });
  }

  // 3. Evidence
  for (const item of detail.evidence) {
    if (seenIds.has(item.evidence_id)) continue;
    seenIds.add(item.evidence_id);

    elements.push({
      group: 'nodes',
      data: {
        id: item.evidence_id,
        label: item.original_filename,
        sublabel: item.source_type || 'EVIDENCE',
        category: 'Evidence',
        confidence: null,
        rawProperties: {
          file_size_bytes: item.file_size_bytes,
          sha256_hash: item.sha256_hash,
          source: item.source,
          ingested_at: item.ingested_at,
        },
      },
    });
  }

  // 4. Relationships
  for (const rel of detail.relationships) {
    const isContradiction = rel.relation_type === 'CONTRADICTS';
    elements.push({
      group: 'edges',
      data: {
        id: rel.relationship_id,
        source: rel.source_id,
        target: rel.target_id,
        label: rel.label,
        relationType: rel.relation_type,
        provenance: rel.provenance,
        isObserved: rel.is_observed,
        isContradiction,
        confidence: rel.weight,
        rawProperties: {
          evidence_basis: rel.evidence_basis,
        },
      },
    });
  }

  return elements;
}
