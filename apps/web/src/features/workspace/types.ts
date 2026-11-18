/**
 * Workspace view models.
 *
 * Field names follow `shared/schemas/README.md` (snake_case) and enum values follow
 * `shared/enums/README.md`. They live here, not in `src/types`, until the team adopts
 * the Phase 0.5 contract proposal (docs/member-3/phase-0-5-canonical-contract-proposal.md).
 * Where the canonical enums have no suitable value the field is `null` and a
 * human-readable `*_label` explains it — enum values are never invented.
 */
import type { CaseStatus, EntityType, EvidenceType } from '../../types';

/** shared/enums/README.md §4 */
export type RelationType =
  | 'COMMUNICATED_WITH'
  | 'PRESENT_AT'
  | 'CO_OCCURRED_WITH'
  | 'OWNS'
  | 'ASSOCIATE_OF'
  | 'CONTRADICTS';

/** shared/enums/README.md §5 (also backend CanonicalTimePrecision) */
export type TimePrecision = 'EXACT_SECOND' | 'MINUTE' | 'HOUR' | 'DAY' | 'ESTIMATED';

/** backend/app/schemas/reconstruction.py TemporalOrderingStatus */
export type TemporalOrderingStatus = 'STRICTLY_ORDERED' | 'CONCURRENT_UNCERTAIN';

/** How information entered the case. `supplied` = documented by a third party, not independently observed. */
export type ProvenanceClass = 'observed' | 'supplied' | 'inferred';

export type ReviewStatus = 'verified' | 'pending' | 'flagged';

export type DataSource = 'synthetic' | 'api';

export interface WorkspaceCase {
  case_id: string;
  title: string;
  fir_number?: string;
  description: string;
  status: CaseStatus;
  created_at: string;
  updated_at: string;
  /** Summary counts. Synthetic for demonstration cases. */
  evidence_count: number;
  event_count: number;
  entity_count: number;
  review_count: number;
  /** Whether detailed records are available for this case. */
  has_detail: boolean;
}

export interface WorkspaceEvidence {
  evidence_id: string;
  case_id: string;
  source_type: EvidenceType | null;
  /** Display label; explains the gap when `source_type` is null. */
  source_type_label: string;
  /** Originating system or device, e.g. "Gate B camera 03". */
  source: string;
  original_filename: string;
  file_size_bytes: number;
  sha256_hash: string;
  ingested_at: string;
  metadata: Record<string, string>;
  extracted_summary: string;
  provenance: ProvenanceClass;
  review: ReviewStatus;
  entity_ids: string[];
  event_ids: string[];
}

export interface WorkspaceEntity {
  entity_id: string;
  entity_type: EntityType | null;
  type_label: string;
  canonical_name: string;
  aliases: string[];
  source_evidence_ids: string[];
  /** Extraction confidence. `null` when not provided — never shown as zero. */
  confidence: number | null;
  attributes: Record<string, string>;
}

export interface UncertaintyWindow {
  window_start_utc: string;
  window_end_utc: string;
  basis: string;
}

export interface WorkspaceEvent {
  event_id: string;
  title: string;
  event_type: string;
  /** Normalised UTC timestamp. */
  timestamp: string;
  timestamp_precision: TimePrecision;
  /** Value exactly as recorded by the source, in its own clock/time zone. */
  source_recorded_time: string;
  source_timezone: string;
  uncertainty: UncertaintyWindow | null;
  ordering_status: TemporalOrderingStatus;
  location_name?: string;
  involved_entity_ids: string[];
  source_evidence_id: string;
  provenance: ProvenanceClass;
}

export interface WorkspaceRelationship {
  relationship_id: string;
  source_id: string;
  target_id: string;
  relation_type: RelationType;
  label: string;
  /** Strength/confidence. `null` when not provided — never shown as zero. */
  weight: number | null;
  is_observed: boolean;
  provenance: ProvenanceClass;
  evidence_basis: string[];
}

export type ReviewCategory = 'conflicting' | 'uncertain' | 'missing' | 'unverified' | 'supporting';

export interface ReviewRecord {
  evidence_id: string;
  label: string;
  value: string;
}

export interface ReviewItem {
  id: string;
  category: ReviewCategory;
  title: string;
  summary: string;
  records: ReviewRecord[];
  discrepancy?: string;
  related_event_ids: string[];
  related_entity_ids: string[];
  status: ReviewStatus;
  suggested_action: string;
  limitations: string[];
}

export type PlaceKind = 'site' | 'gate' | 'camera' | 'access-point' | 'off-plan';

export interface SpatialPlace {
  place_id: string;
  name: string;
  kind: PlaceKind;
  /** Schematic coordinates in the 800×520 site-plan viewBox — not geographic. */
  x: number;
  y: number;
  /** Schematic radius of an area of uncertainty, if the place implies proximity only. */
  uncertainty_radius?: number;
  event_ids: string[];
  evidence_ids: string[];
  note: string;
}

export interface CaseDetail {
  case: WorkspaceCase;
  evidence: WorkspaceEvidence[];
  entities: WorkspaceEntity[];
  events: WorkspaceEvent[];
  relationships: WorkspaceRelationship[];
  review_items: ReviewItem[];
  places: SpatialPlace[];
}
