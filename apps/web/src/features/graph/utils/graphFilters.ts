import type {
  CytoscapeEdgeData,
  CytoscapeElement,
  CytoscapeNodeData,
  ForensicNodeCategory,
  GraphFilterState,
} from '../types/graph';

export const CATEGORY_LABELS: Record<ForensicNodeCategory, string> = {
  Case: 'Case',
  Evidence: 'Evidence Artifact',
  ExtractionRun: 'Extraction Run',
  EntityCandidate: 'Entity Candidate',
  EventCandidate: 'Event Candidate',
  RelationshipAssertion: 'Relationship Assertion',
  ReconstructionRun: 'Reconstruction Run',
  ContradictionLead: 'Contradiction Lead',
  TemporalCluster: 'Temporal Cluster',
  SpatialLocus: 'Spatial Locus',
};

export const INITIAL_FILTER_STATE: GraphFilterState = {
  hiddenCategories: new Set<ForensicNodeCategory>(['ExtractionRun']),
  hiddenEdgeTypes: new Set<string>(['PRODUCED_EXTRACTION_RUN']),
  searchQuery: '',
  showOnlyObserved: false,
  showContradictionsOnly: false,
};

/**
 * Evaluates whether a Cytoscape element passes the active filter criteria.
 */
export function matchesFilters(
  element: CytoscapeElement,
  filters: GraphFilterState,
  visibleNodeIds: Set<string>,
): boolean {
  if (element.group === 'nodes') {
    const node = element.data as CytoscapeNodeData;

    // Category filter
    if (filters.hiddenCategories.has(node.category)) {
      return false;
    }

    // Contradictions-only mode
    if (filters.showContradictionsOnly && node.category !== 'ContradictionLead') {
      return false;
    }

    // Search query matching
    if (filters.searchQuery.trim().length > 0) {
      const q = filters.searchQuery.toLowerCase();
      const matchLabel = node.label.toLowerCase().includes(q);
      const matchSub = node.sublabel.toLowerCase().includes(q);
      const matchId = node.id.toLowerCase().includes(q);
      if (!matchLabel && !matchSub && !matchId) {
        return false;
      }
    }

    return true;
  }

  if (element.group === 'edges') {
    const edge = element.data as CytoscapeEdgeData;

    // Both source and target nodes must be visible
    if (!visibleNodeIds.has(edge.source) || !visibleNodeIds.has(edge.target)) {
      return false;
    }

    // Edge type filter
    if (filters.hiddenEdgeTypes.has(edge.relationType)) {
      return false;
    }

    // Only observed filter
    if (filters.showOnlyObserved && !edge.isObserved) {
      return false;
    }

    // Contradiction filter
    if (filters.showContradictionsOnly && !edge.isContradiction) {
      return false;
    }

    return true;
  }

  return true;
}
