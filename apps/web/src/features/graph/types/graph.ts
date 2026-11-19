/**
 * Forensic Knowledge Graph Domain Models and Backend DTOs.
 *
 * Implements strict type safety for Phase 2A/2B contracts.
 */

export type ForensicNodeCategory =
  | 'Case'
  | 'Evidence'
  | 'ExtractionRun'
  | 'EntityCandidate'
  | 'EventCandidate'
  | 'RelationshipAssertion'
  | 'ReconstructionRun'
  | 'ContradictionLead'
  | 'TemporalCluster'
  | 'SpatialLocus';

export type ForensicEdgeType =
  | 'HAS_EVIDENCE'
  | 'PRODUCED_EXTRACTION_RUN'
  | 'EXTRACTED_ENTITY'
  | 'EXTRACTED_EVENT'
  | 'PRODUCED_ASSERTION'
  | 'COMMUNICATED_WITH'
  | 'PRESENT_AT'
  | 'CO_OCCURRED_WITH'
  | 'OWNS'
  | 'ASSOCIATE_OF'
  | 'PRODUCED_RECONSTRUCTION'
  | 'IDENTIFIED_CONTRADICTION'
  | 'CONTRADICTS_EVENT'
  | 'SOURCE_ENTITY'
  | 'TARGET_ENTITY'
  | 'SUPPORTED_BY_EVIDENCE';

export type ProvenanceKind = 'observed' | 'inferred' | 'structural' | 'supplied';

/** Raw Node representation returned by Rust backend graph endpoints. */
export interface BackendGraphNode {
  id: string;
  label: string;
  properties: Record<string, unknown>;
}

/** Raw Edge representation returned by Rust backend graph endpoints. */
export interface BackendGraphRelationship {
  id: string;
  source: string;
  target: string;
  relation_type: string;
  properties: Record<string, unknown>;
}

/** DTO for `GET /api/v1/cases/:case_id/graph/summary`. */
export interface GraphSummaryDto {
  case_id: string;
  evidence_count: number;
  extraction_run_count: number;
  entity_candidate_count: number;
  event_candidate_count: number;
  relationship_assertion_count: number;
  reconstruction_run_count: number;
  contradiction_lead_count: number;
  graph_synchronization_status: string;
  last_projection_time: string | null;
  identifier_privacy: string;
}

/** DTO for `GET /api/v1/cases/:case_id/graph/entities/:entity_id/neighbors`. */
export interface GraphNeighborResponseDto {
  case_id: string;
  root_entity_id: string;
  depth: number;
  nodes: BackendGraphNode[];
  relationships: BackendGraphRelationship[];
}

/** DTO for `GET /api/v1/cases/:case_id/graph/path`. */
export interface GraphPathResponseDto {
  case_id: string;
  from_entity_id: string;
  to_entity_id: string;
  found: boolean;
  nodes: BackendGraphNode[];
  relationships: BackendGraphRelationship[];
}

/** Normalised Cytoscape-compatible node definition. */
export interface CytoscapeNodeData {
  id: string;
  label: string;
  sublabel: string;
  category: ForensicNodeCategory;
  isRoot?: boolean;
  isOnPath?: boolean;
  isRedacted?: boolean;
  confidence?: number | null;
  rawProperties: Record<string, unknown>;
}

/** Normalised Cytoscape-compatible edge definition. */
export interface CytoscapeEdgeData {
  id: string;
  source: string;
  target: string;
  label: string;
  relationType: string;
  provenance: ProvenanceKind;
  isObserved: boolean;
  isContradiction: boolean;
  isOnPath?: boolean;
  confidence?: number | null;
  rawProperties: Record<string, unknown>;
}

export interface CytoscapeElement {
  group: 'nodes' | 'edges';
  data: CytoscapeNodeData | CytoscapeEdgeData;
}

export type GraphSyncStatusKind =
  | 'SYNCHRONIZED'
  | 'SYNCING'
  | 'DEGRADED'
  | 'STANDALONE_GRAPH'
  | 'GRAPH_DISABLED'
  | 'UNKNOWN';

export interface GraphFilterState {
  hiddenCategories: Set<ForensicNodeCategory>;
  hiddenEdgeTypes: Set<string>;
  searchQuery: string;
  showOnlyObserved: boolean;
  showContradictionsOnly: boolean;
}
