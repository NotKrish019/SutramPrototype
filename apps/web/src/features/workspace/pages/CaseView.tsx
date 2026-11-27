import React from 'react';
import { FileQuestion, FolderX, MonitorPlay } from 'lucide-react';
import { Link } from '../../../router/Link';
import { ROUTES, caseHref, type CaseSection } from '../../../router/router';
import { useCaseDetail, useIsSynthetic, useWorkspaceRepository } from '../data/WorkspaceDataContext';
import { useWorkspaceShell } from '../layout/WorkspaceShellContext';
import { CASE_NAV, WORKFLOW_NAV } from '../layout/navigation';
import { CaseStatusBadge, EmptyState, ErrorState, LoadingState, SyntheticTag } from '../components/ui';
import { formatDateTime } from '../format';
import type { CaseDetail, WorkspaceCase } from '../types';
import { CaseOverviewSection } from './CaseOverviewSection';
import { ContradictionReview } from './ContradictionReview';
import { CorrelationWorkspace } from './CorrelationWorkspace';
import { EventReconstruction } from './EventReconstruction';
import { EvidenceRepository } from './EvidenceRepository';
import { KnowledgeGraphWorkspace } from './KnowledgeGraphWorkspace';
import { ReportsWorkspace } from './ReportsWorkspace';
import { SpatialReconstruction } from './SpatialReconstruction';
import { VideoIntelligence } from './VideoIntelligence';
import { ReconstructionTheatre } from '../../theatre/ReconstructionTheatre';

const SECTIONS: Record<CaseSection, React.FC<{ detail: CaseDetail }>> = {
  overview: CaseOverviewSection,
  evidence: EvidenceRepository,
  correlation: CorrelationWorkspace,
  graph: KnowledgeGraphWorkspace,
  timeline: EventReconstruction,
  spatial: SpatialReconstruction,
  contradictions: ContradictionReview,
  video: VideoIntelligence,
  reports: ReportsWorkspace,
  theatre: ReconstructionTheatre,
};

const CaseHeader: React.FC<{ item: WorkspaceCase; section: CaseSection; synthetic: boolean }> = ({ item, section, synthetic }) => {
  const current = CASE_NAV.find((navItem) => navItem.section === section);
  const tabs = [{ section: 'overview' as CaseSection, short: 'Overview', step: undefined as number | undefined, purpose: 'Investigation overview' }, ...WORKFLOW_NAV];
  return (
    <header className="px-4 pt-6 sm:px-6 lg:px-8">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
        <div className="min-w-0 ws-rise">
          <div className="flex flex-wrap items-center gap-2">
            <CaseStatusBadge status={item.status} />
            {synthetic && <SyntheticTag />}
            <span className="font-mono text-xs text-fg-subtle">{item.case_id}</span>
          </div>
          <h1 className="mt-3 font-serif text-[clamp(1.6rem,2.6vw,2.4rem)] leading-[1.05] text-fg-bright">{item.title}</h1>
          <p className="mt-1.5 text-sm text-fg-subtle">
            {item.fir_number && <>{item.fir_number} · </>}
            {item.evidence_count} evidence · updated {formatDateTime(item.updated_at)}
          </p>
        </div>
        <Link
          to={caseHref(item.case_id, 'theatre')}
          className="theatre-cta group inline-flex h-11 shrink-0 items-center gap-2.5 self-start rounded-full bg-crimson pl-4 pr-5 text-sm font-semibold text-white transition-colors duration-fast hover:bg-crimson-deep focus:outline-none focus-visible:ring-2 focus-visible:ring-white lg:self-end ws-rise"
        >
          <MonitorPlay aria-hidden="true" className="h-4 w-4" />
          Play reconstruction
        </Link>
      </div>
      <nav aria-label="Investigation sections" className="-mx-1 mt-5 overflow-x-auto pb-1">
        <ol className="inline-flex min-w-max items-center gap-1 rounded-full border border-white/[0.07] bg-white/[0.025] p-1">
          {tabs.map((navItem) => {
            const active = navItem.section === section;
            const done = current?.step !== undefined && navItem.step !== undefined && navItem.step < current.step;
            return (
              <li key={navItem.section}>
                <Link
                  to={caseHref(item.case_id, navItem.section)}
                  aria-current={active ? 'page' : undefined}
                  title={navItem.purpose}
                  className={`flex h-9 items-center gap-2 rounded-full px-3.5 text-sm transition-colors duration-fast focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40 ${
                    active ? 'bg-white/[0.1] font-medium text-fg-bright shadow-[inset_0_0_0_1px_rgba(255,255,255,0.08)]' : 'text-fg-subtle hover:text-fg'
                  }`}
                >
                  {navItem.step !== undefined && (
                    <span
                      aria-hidden="true"
                      className={`flex h-5 w-5 items-center justify-center rounded-full text-[0.65rem] tabular-nums ${
                        active ? 'bg-crimson text-white' : done ? 'bg-white/10 text-fg-muted' : 'border border-white/10'
                      }`}
                    >
                      {navItem.step}
                    </span>
                  )}
                  {navItem.short}
                </Link>
              </li>
            );
          })}
        </ol>
      </nav>
    </header>
  );
};

export const CaseView: React.FC<{ caseId: string; section: CaseSection }> = ({ caseId, section }) => {
  const { cases } = useWorkspaceShell();
  const repository = useWorkspaceRepository();
  const detail = useCaseDetail(caseId);
  const summary = cases.state.status === 'success' ? cases.state.data.find((c) => c.case_id === caseId) : undefined;
  const synthetic = useIsSynthetic(detail.state.data?.case ?? summary);

  if (detail.state.status === 'loading' || cases.state.status === 'loading') {
    return <LoadingState label={`Loading ${caseId}`} />;
  }
  if (detail.state.status === 'error') {
    return <ErrorState message={detail.state.error} onRetry={detail.retry} />;
  }
  if (cases.state.status === 'error') {
    return <ErrorState message={cases.state.error} onRetry={cases.retry} />;
  }

  const data = detail.state.data;
  const item = data?.case ?? summary;

  if (!item) {
    return (
      <EmptyState
        icon={FolderX}
        title="Case not found"
        action={
          <Link to={ROUTES.workspace} className="font-mono text-micro uppercase text-signal-accent hover:underline">
            Back to investigations
          </Link>
        }
      >
        There is no case with the identifier <code className="font-mono text-fg">{caseId}</code>.
      </EmptyState>
    );
  }

  const Section = SECTIONS[section];
  if (section === 'theatre' && data) {
    // The theatre is a full-bleed stage with its own header and controls.
    return (
      <div key={`${caseId}-theatre`} className="ws-view-enter">
        <ReconstructionTheatre detail={data} />
      </div>
    );
  }
  return (
    <div>
      <CaseHeader item={item} section={section} synthetic={synthetic} />
      {data ? (
        <div key={`${caseId}-${section}`} className="ws-view-enter">
          <Section detail={data} />
        </div>
      ) : (
        <EmptyState icon={FileQuestion} title="Detailed records not available">
          {synthetic ? (
            <>
              {item.case_id} is a summary-only synthetic case. Detailed demonstration records are provided for{' '}
              {repository.defaultCaseId && (
                <Link to={caseHref(repository.defaultCaseId, section)} className="text-signal-accent hover:underline">
                  {repository.defaultCaseId}
                </Link>
              )}
              .
            </>
          ) : (
            'No detailed records were returned for this case.'
          )}
        </EmptyState>
      )}
    </div>
  );
};
