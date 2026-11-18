/**
 * CASE-026 — Warehouse Access Investigation.
 *
 * ENTIRELY SYNTHETIC demonstration data for the public site. No real people,
 * places, devices or records. Not produced by any processing pipeline.
 * Types are local to the landing feature and intentionally do not mirror the
 * shared contracts in `shared/schemas/`.
 */

/** How a piece of information entered the case. */
export type ProvenanceClass = 'observed' | 'supplied' | 'inferred';

export type ReviewStatus = 'verified' | 'pending' | 'flagged';

export type EvidenceKind = 'cctv' | 'access-log' | 'communication' | 'transaction' | 'network';

export type EntityKind = 'person' | 'device' | 'vehicle' | 'location' | 'account' | 'credential';

export interface DemoEvidence {
  id: string;
  kind: EvidenceKind;
  label: string;
  fileName: string;
  /** Truncated, fictional digest recorded at ingestion. */
  sha256: string;
  acquiredAt: string;
  extracted: string;
  provenance: ProvenanceClass;
  review: ReviewStatus;
  eventIds: string[];
  entityIds: string[];
}

export interface DemoEntity {
  id: string;
  kind: EntityKind;
  label: string;
  detail: string;
  /** Position inside the 600×380 graph viewBox. */
  x: number;
  y: number;
}

export interface DemoRelationship {
  id: string;
  from: string;
  to: string;
  label: string;
  provenance: ProvenanceClass;
}

export interface DemoEvent {
  id: string;
  time: string;
  title: string;
  evidenceId: string;
  provenance: ProvenanceClass;
  note?: string;
}

export interface DemoDiscrepancy {
  id: string;
  title: string;
  summary: string;
  recordA: { evidenceId: string; label: string; value: string };
  recordB: { evidenceId: string; label: string; value: string };
  variance: string;
  action: string;
}

export interface DemoCase {
  id: string;
  title: string;
  status: string;
  evidence: DemoEvidence[];
  entities: DemoEntity[];
  relationships: DemoRelationship[];
  events: DemoEvent[];
  discrepancies: DemoDiscrepancy[];
}

export const DEMO_CASE: DemoCase = {
  id: 'CASE-026',
  title: 'Warehouse Access Investigation',
  status: 'Under review',
  evidence: [
    {
      id: 'EV-01',
      kind: 'cctv',
      label: 'CCTV · Gate B camera 03',
      fileName: 'gateB_cam03_2026-03-14.mp4',
      sha256: '9f2c…a41e',
      acquiredAt: '2026-03-16 10:12 UTC',
      extracted: 'Vehicle with partial plate "…47" stops at Gate B; gate opens at 22:41:07 (camera clock).',
      provenance: 'observed',
      review: 'flagged',
      eventIds: ['EVT-3'],
      entityIds: ['ENT-VEH', 'ENT-LOC'],
    },
    {
      id: 'EV-02',
      kind: 'access-log',
      label: 'Access control log',
      fileName: 'door_controller_export.csv',
      sha256: '3b77…0c92',
      acquiredAt: '2026-03-16 10:40 UTC',
      extracted: 'Badge 4471 granted entry at Gate B, 22:43:52 (controller clock).',
      provenance: 'observed',
      review: 'flagged',
      eventIds: ['EVT-4'],
      entityIds: ['ENT-BADGE', 'ENT-LOC'],
    },
    {
      id: 'EV-03',
      kind: 'communication',
      label: 'Communication export',
      fileName: 'device_D01_chat_export.json',
      sha256: 'c5e1…77bd',
      acquiredAt: '2026-03-17 09:05 UTC',
      extracted: 'Message "at the gate in 5" sent from device D-01 at 22:36:14.',
      provenance: 'observed',
      review: 'verified',
      eventIds: ['EVT-2'],
      entityIds: ['ENT-DEV', 'ENT-PA'],
    },
    {
      id: 'EV-04',
      kind: 'transaction',
      label: 'Transaction record',
      fileName: 'card_statement_mar2026.pdf',
      sha256: '71aa…e3f0',
      acquiredAt: '2026-03-18 14:22 UTC',
      extracted: 'Card ending 8812 used at a fuel station, 21:58 (statement records minutes only).',
      provenance: 'supplied',
      review: 'verified',
      eventIds: ['EVT-1'],
      entityIds: ['ENT-ACC', 'ENT-PA'],
    },
    {
      id: 'EV-05',
      kind: 'network',
      label: 'Network log',
      fileName: 'warehouse_ap_assoc.log',
      sha256: 'e04d…5b19',
      acquiredAt: '2026-03-16 11:30 UTC',
      extracted: 'Device MAC matching D-01 associated with warehouse access point at 22:44:10.',
      provenance: 'observed',
      review: 'pending',
      eventIds: ['EVT-5'],
      entityIds: ['ENT-DEV', 'ENT-LOC'],
    },
  ],
  entities: [
    { id: 'ENT-PA', kind: 'person', label: 'Person A', detail: 'Badge holder (HR record)', x: 300, y: 72 },
    { id: 'ENT-DEV', kind: 'device', label: 'Device D-01', detail: 'Mobile handset', x: 120, y: 160 },
    { id: 'ENT-BADGE', kind: 'credential', label: 'Badge 4471', detail: 'Access credential', x: 480, y: 160 },
    { id: 'ENT-VEH', kind: 'vehicle', label: 'Vehicle V-1', detail: 'Partial plate "…47"', x: 470, y: 300 },
    { id: 'ENT-LOC', kind: 'location', label: 'Warehouse 7 · Gate B', detail: 'Site', x: 300, y: 222 },
    { id: 'ENT-ACC', kind: 'account', label: 'Card ··8812', detail: 'Payment card', x: 128, y: 310 },
  ],
  relationships: [
    { id: 'R1', from: 'ENT-PA', to: 'ENT-BADGE', label: 'issued to', provenance: 'supplied' },
    { id: 'R2', from: 'ENT-PA', to: 'ENT-DEV', label: 'likely user of', provenance: 'inferred' },
    { id: 'R3', from: 'ENT-DEV', to: 'ENT-LOC', label: 'associated at', provenance: 'observed' },
    { id: 'R4', from: 'ENT-BADGE', to: 'ENT-LOC', label: 'entry at', provenance: 'observed' },
    { id: 'R5', from: 'ENT-VEH', to: 'ENT-LOC', label: 'recorded at', provenance: 'observed' },
    { id: 'R6', from: 'ENT-ACC', to: 'ENT-PA', label: 'held by', provenance: 'supplied' },
    { id: 'R7', from: 'ENT-VEH', to: 'ENT-PA', label: 'possible link', provenance: 'inferred' },
  ],
  events: [
    { id: 'EVT-1', time: '21:58', title: 'Card payment at fuel station', evidenceId: 'EV-04', provenance: 'supplied' },
    { id: 'EVT-2', time: '22:36:14', title: 'Message sent from device D-01', evidenceId: 'EV-03', provenance: 'observed' },
    {
      id: 'EVT-3',
      time: '22:41:07',
      title: 'Gate B opens · vehicle present',
      evidenceId: 'EV-01',
      provenance: 'observed',
      note: 'Camera clock — offset not yet verified',
    },
    { id: 'EVT-4', time: '22:43:52', title: 'Badge 4471 granted entry', evidenceId: 'EV-02', provenance: 'observed' },
    { id: 'EVT-5', time: '22:44:10', title: 'D-01 joins warehouse access point', evidenceId: 'EV-05', provenance: 'observed' },
  ],
  discrepancies: [
    {
      id: 'DX-01',
      title: 'Timestamp variance',
      summary:
        'Timestamp variance detected between an access-control record and a CCTV recording. Clock synchronization requires verification.',
      recordA: { evidenceId: 'EV-01', label: 'CCTV gate opening', value: '22:41:07' },
      recordB: { evidenceId: 'EV-02', label: 'Access log entry', value: '22:43:52' },
      variance: '+2 min 45 s',
      action: 'Obtain camera NTP configuration and controller clock audit before ordering these events.',
    },
  ],
};

export const PROVENANCE_LABEL: Record<ProvenanceClass, string> = {
  observed: 'Observed',
  supplied: 'Supplied',
  inferred: 'Inferred',
};

export const PROVENANCE_DESCRIPTION: Record<ProvenanceClass, string> = {
  observed: 'Recorded directly in a source artifact.',
  supplied: 'Provided by a party or record holder; not independently observed.',
  inferred: 'Derived by correlation. Requires investigator review; not proof.',
};

export const REVIEW_LABEL: Record<ReviewStatus, string> = {
  verified: 'Reviewed',
  pending: 'Pending review',
  flagged: 'Flagged for verification',
};

export function findEvidence(id: string): DemoEvidence | undefined {
  return DEMO_CASE.evidence.find((item) => item.id === id);
}

export function findEntity(id: string): DemoEntity | undefined {
  return DEMO_CASE.entities.find((item) => item.id === id);
}
