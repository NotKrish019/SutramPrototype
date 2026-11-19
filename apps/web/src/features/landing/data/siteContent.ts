/** Copy and structure for the public YUKTI site. Anchors are the single source of truth for nav + tests. */

export const SECTION_IDS = {
  capabilities: 'capabilities',
  correlation: 'correlation',
  reconstruction: 'reconstruction',
  timeline: 'timeline',
  contradictions: 'contradictions',
  provenance: 'provenance',
  technology: 'technology',
  platform: 'platform',
  about: 'about',
} as const;

export interface NavItem {
  label: string;
  href: `#${string}`;
}

export const NAV_ITEMS: NavItem[] = [
  { label: 'Platform', href: `#${SECTION_IDS.platform}` },
  { label: 'Capabilities', href: `#${SECTION_IDS.capabilities}` },
  { label: 'Reconstruction', href: `#${SECTION_IDS.reconstruction}` },
  { label: 'Technology', href: `#${SECTION_IDS.technology}` },
  { label: 'About', href: `#${SECTION_IDS.about}` },
];

/** The hero's capability strip — each item jumps to the part of the story that shows it. */
export const CAPABILITY_STRIP: { id: string; label: string; href: `#${string}` }[] = [
  { id: 'multi-source', label: 'Multi-source evidence', href: `#${SECTION_IDS.capabilities}` },
  { id: 'correlation', label: 'Cross-source correlation', href: `#${SECTION_IDS.correlation}` },
  { id: 'spatial', label: 'Spatial reconstruction', href: `#${SECTION_IDS.reconstruction}` },
  { id: 'timeline', label: 'Timeline reconstruction', href: `#${SECTION_IDS.timeline}` },
  { id: 'contradiction', label: 'Contradiction detection', href: `#${SECTION_IDS.contradictions}` },
  { id: 'provenance', label: 'Evidence provenance', href: `#${SECTION_IDS.provenance}` },
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
