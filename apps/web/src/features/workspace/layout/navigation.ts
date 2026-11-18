import {
  AlertTriangle,
  Archive,
  BookOpen,
  Combine,
  FileText,
  FolderOpen,
  Globe,
  HelpCircle,
  LayoutGrid,
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
}

/** Every item maps to an implemented view under /workspace/cases/:caseId. */
export const CASE_NAV: CaseNavItem[] = [
  { section: 'overview', label: 'Case Overview', short: 'Overview', icon: FolderOpen },
  { section: 'evidence', label: 'Evidence Repository', short: 'Evidence', icon: Archive },
  { section: 'correlation', label: 'Evidence Correlation', short: 'Correlation', icon: Combine },
  { section: 'graph', label: 'Knowledge Graph', short: 'Graph', icon: Share2 },
  { section: 'timeline', label: 'Event Reconstruction', short: 'Timeline', icon: ListOrdered },
  { section: 'spatial', label: 'Spatial Reconstruction', short: 'Spatial', icon: Map },
  { section: 'contradictions', label: 'Contradiction Review', short: 'Contradictions', icon: AlertTriangle },
  { section: 'reports', label: 'Reports', short: 'Reports', icon: FileText },
];

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
