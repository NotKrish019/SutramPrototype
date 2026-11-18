/**
 * SYNTHETIC DEMONSTRATION DATA for the investigator workspace.
 *
 * Fictional cases, people, devices, places and records. Nothing here was produced by
 * a forensic processing pipeline. CASE-026 is derived from the shared fixture in
 * `case026.ts` so the public site and the workspace tell the same story.
 */
import type { CaseDetail, WorkspaceCase, WorkspaceEntity, WorkspaceEvent, WorkspaceEvidence } from '../../features/workspace/types';
import { DEMO_CASE } from './case026';

/** Fictional full-length SHA-256 digests; prefixes/suffixes match the truncated values in case026. */
const DIGESTS: Record<string, string> = {
  'EV-01': '9f2c4be1d07a35c8e6f19b2a7d50c3e8814f6a2bd9e07c51f3a8e26b4d17a41e',
  'EV-02': '3b77e0a95c12d4f86b3e1a7c09d5f2e4a68b17c3e95d0f2a4c71b8e36d500c92',
  'EV-03': 'c5e1a3f7092b6d4e8c15f0a97e3b2d6c4f81a5e09d7c3b62f4e8a1d05b9c77bd',
  'EV-04': '71aa0c3e5f9b2d8a64e17c05b3f9d2a8e61c4b07f5a3d9e28c1b6f40a7d2e3f0',
  'EV-05': 'e04d7b1c39f5a2e86d0c4b17f3a9e5d2c86b0a4f17e3d9c25b8a6e1f04c75b19',
};

const EVIDENCE_EXTRA: Record<string, Pick<WorkspaceEvidence, 'source_type' | 'source_type_label' | 'source' | 'file_size_bytes' | 'ingested_at' | 'metadata'>> = {
  'EV-01': {
    source_type: 'CCTV',
    source_type_label: 'CCTV',
    source: 'Gate B camera 03',
    file_size_bytes: 418_302_116,
    ingested_at: '2026-03-16T10:12:44Z',
    metadata: { codec: 'H.264', resolution: '1920×1080', duration: '00:42:13', 'clock source': 'Camera internal clock (NTP not confirmed)' },
  },
  'EV-02': {
    source_type: null,
    source_type_label: 'Access-control log · no canonical EvidenceType',
    source: 'Door controller DC-07',
    file_size_bytes: 2_184_330,
    ingested_at: '2026-03-16T10:40:02Z',
    metadata: { format: 'CSV export', rows: '6,212', 'clock source': 'Controller NTP-synchronised (per facility record)' },
  },
  'EV-03': {
    source_type: 'CHAT_EXPORT',
    source_type_label: 'CHAT_EXPORT',
    source: 'Device D-01 extraction',
    file_size_bytes: 9_771_204,
    ingested_at: '2026-03-17T09:05:31Z',
    metadata: { format: 'JSON', messages: '1,348', 'clock source': 'Handset clock' },
  },
  'EV-04': {
    source_type: 'FINANCIAL_LEDGER',
    source_type_label: 'FINANCIAL_LEDGER',
    source: 'Issuing bank statement',
    file_size_bytes: 512_880,
    ingested_at: '2026-03-18T14:22:10Z',
    metadata: { format: 'PDF', pages: '4', 'time resolution': 'Minutes' },
  },
  'EV-05': {
    source_type: null,
    source_type_label: 'Network log · no canonical EvidenceType',
    source: 'Warehouse Wi-Fi controller',
    file_size_bytes: 31_406_771,
    ingested_at: '2026-03-16T11:30:55Z',
    metadata: { format: 'Syslog', lines: '48,903', 'clock source': 'Controller NTP-synchronised' },
  },
};

const ENTITY_EXTRA: Record<string, Pick<WorkspaceEntity, 'entity_type' | 'type_label' | 'aliases' | 'attributes'>> = {
  'ENT-PA': { entity_type: 'PERSON', type_label: 'PERSON', aliases: ['Badge holder 4471'], attributes: { role: 'Badge holder per HR record' } },
  'ENT-DEV': { entity_type: 'DEVICE', type_label: 'DEVICE', aliases: ['Handset D-01'], attributes: { identifier: 'IMEI ··0417 / MAC ··3a:91' } },
  'ENT-BADGE': { entity_type: 'DEVICE', type_label: 'DEVICE (access credential)', aliases: [], attributes: { serial: '4471', issuer: 'Site security office' } },
  'ENT-VEH': { entity_type: 'VEHICLE', type_label: 'VEHICLE', aliases: [], attributes: { plate: 'Partial — "…47" (unreadable remainder)', colour: 'Light (lighting-dependent)' } },
  'ENT-LOC': { entity_type: 'LOCATION', type_label: 'LOCATION', aliases: ['Gate B'], attributes: { site: 'Warehouse 7', access: 'Controlled vehicle and pedestrian gate' } },
  'ENT-ACC': { entity_type: null, type_label: 'Payment card · no canonical EntityType', aliases: [], attributes: { last4: '8812' } },
};

/** IST is UTC+05:30. Recorded values are kept exactly as the source wrote them. */
const EVENT_EXTRA: Record<string, Omit<WorkspaceEvent, 'event_id' | 'title' | 'source_evidence_id' | 'provenance' | 'involved_entity_ids'>> = {
  'EVT-1': {
    event_type: 'Payment',
    timestamp: '2026-03-14T16:28:00Z',
    timestamp_precision: 'MINUTE',
    source_recorded_time: '14-03-2026 21:58',
    source_timezone: 'Asia/Kolkata (IST, UTC+05:30)',
    uncertainty: { window_start_utc: '2026-03-14T16:28:00Z', window_end_utc: '2026-03-14T16:28:59Z', basis: 'Statement records minutes only' },
    ordering_status: 'STRICTLY_ORDERED',
    location_name: 'Fuel station (off-plan)',
  },
  'EVT-2': {
    event_type: 'Communication',
    timestamp: '2026-03-14T17:06:14Z',
    timestamp_precision: 'EXACT_SECOND',
    source_recorded_time: '2026-03-14 22:36:14 +0530',
    source_timezone: 'Asia/Kolkata (handset setting)',
    uncertainty: null,
    ordering_status: 'STRICTLY_ORDERED',
  },
  'EVT-3': {
    event_type: 'Physical access',
    timestamp: '2026-03-14T17:11:07Z',
    timestamp_precision: 'EXACT_SECOND',
    source_recorded_time: '14/03/2026 22:41:07 (camera overlay)',
    source_timezone: 'Assumed IST — camera time-zone setting not confirmed',
    uncertainty: {
      window_start_utc: '2026-03-14T17:08:07Z',
      window_end_utc: '2026-03-14T17:14:07Z',
      basis: 'Camera clock not NTP-verified; ±3 min investigator estimate pending clock audit',
    },
    ordering_status: 'CONCURRENT_UNCERTAIN',
    location_name: 'Warehouse 7 · Gate B',
  },
  'EVT-4': {
    event_type: 'Physical access',
    timestamp: '2026-03-14T17:13:52Z',
    timestamp_precision: 'EXACT_SECOND',
    source_recorded_time: '2026-03-14T17:13:52Z (controller, UTC)',
    source_timezone: 'UTC (controller)',
    uncertainty: null,
    ordering_status: 'CONCURRENT_UNCERTAIN',
    location_name: 'Warehouse 7 · Gate B',
  },
  'EVT-5': {
    event_type: 'Network association',
    timestamp: '2026-03-14T17:14:10Z',
    timestamp_precision: 'EXACT_SECOND',
    source_recorded_time: 'Mar 14 17:14:10 (syslog, UTC)',
    source_timezone: 'UTC (controller)',
    uncertainty: null,
    ordering_status: 'CONCURRENT_UNCERTAIN',
    location_name: 'Warehouse 7 · AP-WH7 coverage',
  },
};

const EVENT_ENTITIES: Record<string, string[]> = {
  'EVT-1': ['ENT-ACC', 'ENT-PA'],
  'EVT-2': ['ENT-DEV', 'ENT-PA'],
  'EVT-3': ['ENT-VEH', 'ENT-LOC'],
  'EVT-4': ['ENT-BADGE', 'ENT-LOC'],
  'EVT-5': ['ENT-DEV', 'ENT-LOC'],
};

const RELATION_TYPES: Record<string, 'OWNS' | 'PRESENT_AT' | 'CO_OCCURRED_WITH'> = {
  R1: 'OWNS',
  R2: 'OWNS',
  R3: 'PRESENT_AT',
  R4: 'PRESENT_AT',
  R5: 'PRESENT_AT',
  R6: 'OWNS',
  R7: 'CO_OCCURRED_WITH',
};

const RELATION_BASIS: Record<string, string[]> = {
  R1: [],
  R2: ['EV-03', 'EV-05'],
  R3: ['EV-05'],
  R4: ['EV-02'],
  R5: ['EV-01'],
  R6: ['EV-04'],
  R7: ['EV-01', 'EV-02'],
};

const evidence: WorkspaceEvidence[] = DEMO_CASE.evidence.map((item) => ({
  evidence_id: item.id,
  case_id: DEMO_CASE.id,
  ...EVIDENCE_EXTRA[item.id]!,
  original_filename: item.fileName,
  sha256_hash: DIGESTS[item.id]!,
  extracted_summary: item.extracted,
  provenance: item.provenance,
  review: item.review,
  entity_ids: item.entityIds,
  event_ids: item.eventIds,
}));

const entities: WorkspaceEntity[] = DEMO_CASE.entities.map((entity) => ({
  entity_id: entity.id,
  canonical_name: entity.label,
  ...ENTITY_EXTRA[entity.id]!,
  source_evidence_ids: DEMO_CASE.evidence.filter((e) => e.entityIds.includes(entity.id)).map((e) => e.id),
  confidence: null,
}));

const events: WorkspaceEvent[] = DEMO_CASE.events.map((event) => ({
  event_id: event.id,
  title: event.title,
  ...EVENT_EXTRA[event.id]!,
  involved_entity_ids: EVENT_ENTITIES[event.id] ?? [],
  source_evidence_id: event.evidenceId,
  provenance: event.provenance,
}));

const CASE_026: WorkspaceCase = {
  case_id: DEMO_CASE.id,
  title: DEMO_CASE.title,
  fir_number: 'FIR-SYN-0326/2026',
  description:
    'Unscheduled after-hours entry at Warehouse 7, Gate B. Footage, access-control, device, network and financial records are being correlated to establish the sequence of events and identify items requiring verification.',
  status: 'UNDER_REVIEW',
  created_at: '2026-03-15T08:30:00Z',
  updated_at: '2026-03-18T15:02:00Z',
  evidence_count: evidence.length,
  event_count: events.length,
  entity_count: entities.length,
  review_count: 0, // filled below from review items
  has_detail: true,
};

const detail026: CaseDetail = {
  case: CASE_026,
  evidence,
  entities,
  events,
  relationships: [
    ...DEMO_CASE.relationships.map((rel) => ({
      relationship_id: rel.id,
      source_id: rel.from,
      target_id: rel.to,
      relation_type: RELATION_TYPES[rel.id]!,
      label: rel.label,
      weight: null,
      is_observed: rel.provenance !== 'inferred',
      provenance: rel.provenance,
      evidence_basis: RELATION_BASIS[rel.id] ?? [],
    })),
    {
      relationship_id: 'R8',
      source_id: 'EVT-3',
      target_id: 'EVT-4',
      relation_type: 'CONTRADICTS',
      label: 'timestamp conflict',
      weight: null,
      is_observed: false,
      provenance: 'inferred',
      evidence_basis: ['EV-01', 'EV-02'],
    },
  ],
  review_items: [
    {
      id: 'DX-01',
      category: 'conflicting',
      title: 'Gate B timestamps disagree',
      summary:
        'Timestamp variance detected between an access-control record and a CCTV recording. Clock synchronization requires verification.',
      records: [
        { evidence_id: 'EV-01', label: 'CCTV gate opening (camera clock)', value: '22:41:07 IST' },
        { evidence_id: 'EV-02', label: 'Access-control entry (controller, UTC)', value: '22:43:52 IST' },
      ],
      discrepancy: '+2 min 45 s between sources describing the same gate',
      related_event_ids: ['EVT-3', 'EVT-4'],
      related_entity_ids: ['ENT-LOC', 'ENT-VEH', 'ENT-BADGE'],
      status: 'pending',
      suggested_action: 'Obtain the camera NTP configuration and the controller clock audit before ordering these events.',
      limitations: [
        'The two records may describe different gate cycles.',
        'Camera overlay time zone is assumed, not confirmed.',
      ],
    },
    {
      id: 'DX-02',
      category: 'uncertain',
      title: 'Person A as user of device D-01',
      summary: 'Link proposed from sender metadata (EV-03) and Wi-Fi association (EV-05). No subscriber or attribution record has been examined.',
      records: [
        { evidence_id: 'EV-03', label: 'Sender profile name', value: 'Matches Person A' },
        { evidence_id: 'EV-05', label: 'Device MAC at AP-WH7', value: '22:44:10 IST' },
      ],
      related_event_ids: ['EVT-2', 'EVT-5'],
      related_entity_ids: ['ENT-PA', 'ENT-DEV'],
      status: 'pending',
      suggested_action: 'Request subscriber records for D-01 before relying on this link.',
      limitations: ['Profile names are user-editable.', 'Shared or borrowed devices are not excluded.'],
    },
    {
      id: 'DX-03',
      category: 'missing',
      title: 'No interior footage after 22:42',
      summary: 'Camera 03 export ends at 22:42:30 (camera clock). Activity inside the gate after that time is not covered by any ingested footage.',
      records: [{ evidence_id: 'EV-01', label: 'Export end', value: '22:42:30 (camera clock)' }],
      related_event_ids: ['EVT-3'],
      related_entity_ids: ['ENT-LOC'],
      status: 'pending',
      suggested_action: 'Ask the site operator whether additional cameras cover the loading bay.',
      limitations: ['Absence of footage is not evidence of absence of activity.'],
    },
    {
      id: 'DX-04',
      category: 'unverified',
      title: 'Vehicle V-1 identity',
      summary: 'Only the last two plate characters ("…47") are legible. The vehicle is not identified.',
      records: [{ evidence_id: 'EV-01', label: 'Plate read', value: '"…47" (partial)' }],
      related_event_ids: ['EVT-3'],
      related_entity_ids: ['ENT-VEH'],
      status: 'flagged',
      suggested_action: 'Seek corroborating footage from road-side cameras before associating V-1 with any person.',
      limitations: ['Many vehicles share the visible characters.'],
    },
    {
      id: 'DX-05',
      category: 'supporting',
      title: 'Badge entry and device association co-occur',
      summary: 'Badge 4471 entry (EV-02) and D-01 joining AP-WH7 (EV-05) are 18 s apart on NTP-synchronised clocks.',
      records: [
        { evidence_id: 'EV-02', label: 'Badge 4471 entry', value: '22:43:52 IST' },
        { evidence_id: 'EV-05', label: 'D-01 association', value: '22:44:10 IST' },
      ],
      related_event_ids: ['EVT-4', 'EVT-5'],
      related_entity_ids: ['ENT-BADGE', 'ENT-DEV', 'ENT-LOC'],
      status: 'verified',
      suggested_action: 'No action — retained as corroborating context.',
      limitations: ['Co-occurrence does not establish that the same person carried both.'],
    },
  ],
  places: [
    { place_id: 'PL-SITE', name: 'Warehouse 7', kind: 'site', x: 490, y: 250, event_ids: [], evidence_ids: [], note: 'Site boundary and building outline (schematic).' },
    {
      place_id: 'PL-GATE',
      name: 'Gate B',
      kind: 'gate',
      x: 160,
      y: 300,
      event_ids: ['EVT-3', 'EVT-4'],
      evidence_ids: ['EV-01', 'EV-02'],
      note: 'Controlled vehicle and pedestrian gate. Door controller DC-07.',
    },
    {
      place_id: 'PL-CAM',
      name: 'Camera 03',
      kind: 'camera',
      x: 214,
      y: 214,
      event_ids: ['EVT-3'],
      evidence_ids: ['EV-01'],
      note: 'Fixed camera facing Gate B. Field of view is approximate.',
    },
    {
      place_id: 'PL-AP',
      name: 'Access point AP-WH7',
      kind: 'access-point',
      x: 470,
      y: 250,
      uncertainty_radius: 120,
      event_ids: ['EVT-5'],
      evidence_ids: ['EV-05'],
      note: 'Association places a device somewhere within radio coverage — not at a point.',
    },
    {
      place_id: 'PL-FUEL',
      name: 'Fuel station',
      kind: 'off-plan',
      x: 730,
      y: 30,
      event_ids: ['EVT-1'],
      evidence_ids: ['EV-04'],
      note: 'Off-plan; merchant location from statement. Distance and direction not to scale.',
    },
  ],
};
detail026.case.review_count = detail026.review_items.filter((item) => item.status !== 'verified').length;

/** Summary-only synthetic cases (no detailed records are provided for these). */
const SUMMARY_ONLY: WorkspaceCase[] = [
  {
    case_id: 'CASE-024',
    title: 'Retail Card Skimming Series',
    description: 'Synthetic summary-only case.',
    status: 'ACTIVE',
    created_at: '2026-02-21T06:15:00Z',
    updated_at: '2026-03-17T11:40:00Z',
    evidence_count: 14,
    event_count: 31,
    entity_count: 12,
    review_count: 3,
    has_detail: false,
  },
  {
    case_id: 'CASE-022',
    title: 'Supplier Invoice Diversion',
    description: 'Synthetic summary-only case.',
    status: 'ACTIVE',
    created_at: '2026-01-30T09:00:00Z',
    updated_at: '2026-03-12T16:05:00Z',
    evidence_count: 9,
    event_count: 22,
    entity_count: 15,
    review_count: 5,
    has_detail: false,
  },
  {
    case_id: 'CASE-027',
    title: 'Courier Account Takeover',
    description: 'Synthetic summary-only case.',
    status: 'DRAFT',
    created_at: '2026-03-18T07:45:00Z',
    updated_at: '2026-03-18T07:45:00Z',
    evidence_count: 2,
    event_count: 0,
    entity_count: 0,
    review_count: 0,
    has_detail: false,
  },
  {
    case_id: 'CASE-011',
    title: 'Phishing Kit Attribution',
    description: 'Synthetic summary-only case.',
    status: 'ARCHIVED',
    created_at: '2025-10-02T10:00:00Z',
    updated_at: '2026-01-09T13:20:00Z',
    evidence_count: 21,
    event_count: 58,
    entity_count: 27,
    review_count: 0,
    has_detail: false,
  },
];

export const SYNTHETIC_CASES: WorkspaceCase[] = [detail026.case, ...SUMMARY_ONLY];

export const SYNTHETIC_CASE_DETAILS: Record<string, CaseDetail> = {
  [detail026.case.case_id]: detail026,
};

export const DEFAULT_CASE_ID = detail026.case.case_id;
