import React from 'react';
import { FileQuestion, FolderX } from 'lucide-react';
import { Link } from '../../../router/Link';
import { ROUTES, caseHref, type CaseSection } from '../../../router/router';
import { useCaseDetail, useWorkspaceRepository } from '../data/WorkspaceDataContext';
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
};

const CaseHeader: React.FC<{ item: WorkspaceCase; section: CaseSection; synthetic: boolean }> = ({ item, section, synthetic }) => {
  const current = CASE_NAV.find((navItem) => navItem.section === section);
  return (
    <header className="border-b border-ink-line">
      <div className="flex flex-col gap-3 px-4 pb-4 pt-5 sm:px-6 lg:flex-row lg:items-end lg:justify-between lg:px-8">
        <div className="min-w-0 ws-rise">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-xs tabular-nums text-fg-muted">{item.case_id}</span>
            <CaseStatusBadge status={item.status} />
            {synthetic && <SyntheticTag />}
          </div>
          <h1 className="mt-2 font-serif text-[clamp(1.5rem,2.4vw,2.1rem)] leading-[1.05] text-fg-bright">{item.title}</h1>
          <p className="mt-1.5 font-mono text-micro uppercase text-fg-subtle">
            {item.fir_number && <>{item.fir_number} · </>}
            <span className="text-fg-muted">{item.evidence_count} evidence</span> · Updated {formatDateTime(item.updated_at)}
          </p>
        </div>
        {current && (
          <p className="max-w-sm text-sm text-fg-muted lg:text-right ws-rise" style={{ ['--i' as string]: 1 }}>
            <span className="block font-mono text-micro uppercase text-fg-subtle">
              {current.step !== undefined ? `Step ${String(current.step).padStart(2, '0')} · ` : ''}
              {current.label}
            </span>
            {current.purpose}
          </p>
        )}
      </div>
      <nav aria-label="Investigation sections" className="overflow-x-auto px-4 pb-3 sm:px-6 lg:px-8">
        <ol className="flex min-w-max items-center">
          <li className="mr-3">
            <Link
              to={caseHref(item.case_id, 'overview')}
              aria-current={section === 'overview' ? 'page' : undefined}
              className={`flex h-9 items-center gap-2 border px-3 font-mono text-micro uppercase transition-colors duration-fast focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent ${
                section === 'overview' ? 'border-signal-live/60 bg-signal-live/10 text-fg-bright' : 'border-ink-line text-fg-muted hover:text-fg'
              }`}
            >
              Overview
            </Link>
          </li>
          {WORKFLOW_NAV.map((navItem, index) => {
            const active = navItem.section === section;
            const done = current?.step !== undefined && navItem.step !== undefined && navItem.step < current.step;
            return (
              <li key={navItem.section} className="flex items-center">
                {index > 0 && (
                  <span aria-hidden="true" className={`h-px w-5 sm:w-8 ${done || active ? 'bg-fg-subtle' : 'bg-ink-line'}`} />
                )}
                <Link
                  to={caseHref(item.case_id, navItem.section)}
                  aria-current={active ? 'page' : undefined}
                  title={navItem.purpose}
                  className={`group flex h-9 items-center gap-2 px-2 font-mono text-micro uppercase transition-colors duration-fast focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent ${
                    active ? 'text-fg-bright' : 'text-fg-subtle hover:text-fg'
                  }`}
                >
                  <span
                    aria-hidden="true"
                    className={`flex h-5 w-5 items-center justify-center rounded-full border text-[0.6rem] tabular-nums transition-colors duration-base ${
                      active ? 'border-signal-live bg-signal-live text-white' : done ? 'border-fg-subtle text-fg-muted' : 'border-ink-strong'
                    }`}
                  >
                    {navItem.step}
                  </span>
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
  const synthetic = repository.source === 'synthetic';

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
