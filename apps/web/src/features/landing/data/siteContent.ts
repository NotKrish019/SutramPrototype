/** Copy and structure for the public YUKTI site. Anchors are the single source of truth for nav + tests. */

export const SECTION_IDS = {
  sources: 'sources',
  platform: 'platform',
  workbench: 'workbench',
  workflow: 'workflow',
  provenance: 'provenance',
  deliverables: 'capabilities',
  about: 'about',
} as const;

export interface NavItem {
  label: string;
  href: `#${string}`;
}

export const NAV_ITEMS: NavItem[] = [
  { label: 'Platform', href: `#${SECTION_IDS.platform}` },
  { label: 'Capabilities', href: `#${SECTION_IDS.deliverables}` },
  { label: 'Workflow', href: `#${SECTION_IDS.workflow}` },
  { label: 'Evidence Intelligence', href: `#${SECTION_IDS.workbench}` },
  { label: 'About', href: `#${SECTION_IDS.about}` },
];

export type SourceId = 'mobile' | 'cctv' | 'email' | 'social' | 'financial' | 'network';

export interface EvidenceSource {
  id: SourceId;
  index: string;
  title: string;
  description: string;
}

export const EVIDENCE_SOURCES: EvidenceSource[] = [
  { id: 'mobile', index: '01', title: 'Mobile Device Evidence', description: 'Messages, calls, app data and locations.' },
  { id: 'cctv', index: '02', title: 'CCTV & Video', description: 'Frames and timestamps — clocks kept uncertain until verified.' },
  { id: 'email', index: '03', title: 'Emails & Communications', description: 'Headers, threads and participants.' },
  { id: 'social', index: '04', title: 'Social Media', description: 'Accounts and interactions as entities.' },
  { id: 'financial', index: '05', title: 'Financial Records', description: 'Transactions on the same timeline.' },
  { id: 'network', index: '06', title: 'Cloud & Network Data', description: 'Logins, IP records and access points.' },
];

export type CapabilityId = 'correlation' | 'graph' | 'timeline' | 'contradiction' | 'explainable';

export interface Capability {
  id: CapabilityId;
  letter: string;
  title: string;
  summary: string;
}

export const CAPABILITIES: Capability[] = [
  { id: 'correlation', letter: 'A', title: 'Evidence Correlation Engine', summary: 'Links records through shared entities, identifiers, times and places.' },
  { id: 'graph', letter: 'B', title: 'Knowledge Graph Generation', summary: 'People, devices, places and events — with provenance on every edge.' },
  { id: 'timeline', letter: 'C', title: 'Temporal & Spatial Reconstruction', summary: 'One true-to-scale chronology, mapped to places, uncertainty kept.' },
  { id: 'contradiction', letter: 'D', title: 'Contradiction & Inconsistency Detection', summary: 'Conflicts surfaced for review — never silently resolved.' },
  { id: 'explainable', letter: 'E', title: 'Explainable Investigative Intelligence', summary: 'Every lead traces back to its artifacts. Inferred ≠ observed.' },
];

export interface WorkflowStep {
  id: 'ingest' | 'extract' | 'correlate' | 'reconstruct';
  index: string;
  title: string;
  description: string;
  detail: string;
}

export const WORKFLOW_STEPS: WorkflowStep[] = [
  {
    id: 'ingest',
    index: '01',
    title: 'Ingest',
    description: 'Collect evidence from every source.',
    detail: 'Originals stay read-only; SHA-256 recorded at intake.',
  },
  {
    id: 'extract',
    index: '02',
    title: 'Extract',
    description: 'Pull out entities, times and events.',
    detail: 'Each value keeps its artifact reference.',
  },
  {
    id: 'correlate',
    index: '03',
    title: 'Correlate',
    description: 'Link records through shared signals.',
    detail: 'Proposed links are labelled inferred.',
  },
  {
    id: 'reconstruct',
    index: '04',
    title: 'Reconstruct',
    description: 'Order events; surface conflicts.',
    detail: 'Conflicts and uncertainty go to review.',
  },
];

export type DeliverableStatus = 'implemented' | 'in-development' | 'prototype' | 'planned';

export interface Deliverable {
  id: string;
  title: string;
  description: string;
  status: DeliverableStatus;
  statusNote: string;
}

/** Status reflects the repository at the time of writing; see docs/architecture/system-overview.md. */
export const DELIVERABLES: Deliverable[] = [
  {
    id: 'correlation-engine',
    title: 'Evidence Correlation Engine',
    description: 'Cross-source linkage with recorded justification.',
    status: 'planned',
    statusNote: 'Planned · correlation module',
  },
  {
    id: 'knowledge-graph',
    title: 'Knowledge Graph Generation',
    description: 'Typed graph, provenance on every edge.',
    status: 'planned',
    statusNote: 'Planned · graph persistence',
  },
  {
    id: 'reconstruction-dashboard',
    title: 'Crime Reconstruction Dashboard',
    description: 'Timeline and spatial views of activity.',
    status: 'in-development',
    statusNote: 'Reconstruction engines in development',
  },
  {
    id: 'event-sequence',
    title: 'Automated Event-Sequence Generator',
    description: 'Normalised sequences with uncertainty and conflict flags.',
    status: 'in-development',
    statusNote: 'Temporal & contradiction engines in development',
  },
  {
    id: 'investigation-platform',
    title: 'Interactive Investigation Platform',
    description: 'Evidence, graph, timeline and provenance in one place.',
    status: 'prototype',
    statusNote: 'UI prototype · synthetic data',
  },
];

export const DELIVERABLE_STATUS_LABEL: Record<DeliverableStatus, string> = {
  implemented: 'Implemented',
  'in-development': 'In development',
  prototype: 'Prototype',
  planned: 'Planned',
};

export { PROJECT_REPOSITORY_URL } from '../../../config/project';
