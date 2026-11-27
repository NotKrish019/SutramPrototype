import {
  AlertTriangle,
  Archive,
  BookOpen,
  Clapperboard,
  Combine,
  FileText,
  FolderOpen,
  Globe,
  HelpCircle,
  LayoutGrid,
  MonitorPlay,
  ListOrdered,
  Map,
  Settings,
  Share2,
  type LucideIcon,
} from 'lucide-react';
import { PROJECT_DOCS_URL } from '../../../config/project';
import { ROUTES, type CaseSection } from '../../../router/router';

export interface CaseNavItem {
  section: CaseSection;
  label: string;
  short: string;
  icon: LucideIcon;
  /** Position in the investigative workflow (evidence → report); analysis tools have none. */
  step?: number;
  /** One-line purpose shown in the workflow stepper and tooltips. */
  purpose: string;
  group: 'case' | 'workflow' | 'analysis';
}

/**
 * Every item maps to an implemented view under /workspace/cases/:caseId. The workflow group is
 * ordered as the investigation itself runs: evidence in, time, place, conflicts, video, report.
 */
export const CASE_NAV: CaseNavItem[] = [
  { section: 'overview', label: 'Investigation Overview', short: 'Overview', icon: FolderOpen, group: 'case',
    purpose: 'What happened, what conflicts, what to do next' },
  { section: 'theatre', label: 'Reconstruction Theatre', short: 'Theatre', icon: MonitorPlay, group: 'case',
    purpose: 'Watch the whole investigation replay, step by step, with explanations' },
  { section: 'evidence', label: 'Evidence Intake', short: 'Intake', icon: Archive, group: 'workflow', step: 1,
    purpose: 'Exhibits, hashes and chain of custody' },
  { section: 'timeline', label: 'Temporal Reconstruction', short: 'Timeline', icon: ListOrdered, group: 'workflow', step: 2,
    purpose: 'Every source on one corrected clock' },
  { section: 'spatial', label: 'Spatial Reconstruction', short: 'Places', icon: Map, group: 'workflow', step: 3,
    purpose: 'Where evidence places each event' },
  { section: 'contradictions', label: 'Contradiction Intelligence', short: 'Conflicts', icon: AlertTriangle, group: 'workflow', step: 4,
    purpose: 'Claims the evidence cannot reconcile' },
  { section: 'video', label: 'Video Intelligence', short: 'Video', icon: Clapperboard, group: 'workflow', step: 5,
    purpose: 'Keyframes, hashes and frame provenance' },
  { section: 'reports', label: 'Judicial Report', short: 'Report', icon: FileText, group: 'workflow', step: 6,
    purpose: 'Evidence to conclusion, with every limit stated' },
  { section: 'graph', label: 'Knowledge Graph', short: 'Graph', icon: Share2, group: 'analysis',
    purpose: 'People, devices and places as connected records' },
  { section: 'correlation', label: 'Evidence Correlation', short: 'Correlation', icon: Combine, group: 'analysis',
    purpose: 'Which exhibits support each other' },
];

export const WORKFLOW_NAV = CASE_NAV.filter((item) => item.group === 'workflow');

export const WORKSPACE_HOME_NAV = { label: 'Investigations', href: ROUTES.workspace, icon: LayoutGrid };

export interface SecondaryNavItem {
  label: string;
  href: string;
  icon: LucideIcon;
  external?: boolean;
}

export const SECONDARY_NAV: SecondaryNavItem[] = [
  { label: 'Public site', href: ROUTES.home, icon: Globe },
  { label: 'Documentation', href: PROJECT_DOCS_URL, icon: BookOpen, external: true },
  { label: 'Settings', href: ROUTES.workspaceSettings, icon: Settings },
  { label: 'Help', href: ROUTES.workspaceHelp, icon: HelpCircle },
];

export function caseSectionLabel(section: CaseSection): string {
  return CASE_NAV.find((item) => item.section === section)?.label ?? section;
}
