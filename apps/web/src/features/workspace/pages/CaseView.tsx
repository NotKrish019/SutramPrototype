import React from 'react';
import { FileQuestion, FolderX } from 'lucide-react';
import { Link } from '../../../router/Link';
import { ROUTES, caseHref, type CaseSection } from '../../../router/router';
import { useCaseDetail, useWorkspaceRepository } from '../data/WorkspaceDataContext';
import { useWorkspaceShell } from '../layout/WorkspaceShellContext';
import { CASE_NAV } from '../layout/navigation';
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

const SECTIONS: Record<CaseSection, React.FC<{ detail: CaseDetail }>> = {
  overview: CaseOverviewSection,
  evidence: EvidenceRepository,
  correlation: CorrelationWorkspace,
  graph: KnowledgeGraphWorkspace,
  timeline: EventReconstruction,
  spatial: SpatialReconstruction,
  contradictions: ContradictionReview,
  reports: ReportsWorkspace,
};

const CaseHeader: React.FC<{ item: WorkspaceCase; section: CaseSection; synthetic: boolean }> = ({ item, section, synthetic }) => (
  <header className="border-b border-ink-line">
    <div className="flex flex-col gap-4 px-4 pb-4 pt-5 sm:px-6 lg:flex-row lg:items-end lg:justify-between lg:px-8 lg:pt-6">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-mono text-xs text-fg-muted">{item.case_id}</span>
          <CaseStatusBadge status={item.status} />
          {synthetic && <SyntheticTag />}
        </div>
        <h1 className="mt-2 font-serif text-[clamp(1.8rem,3vw,2.5rem)] font-normal leading-[1.02] tracking-[-0.015em] text-fg-bright">{item.title}</h1>
        <p className="mt-2 font-mono text-micro uppercase text-fg-subtle">
          {item.fir_number && <>{item.fir_number} · </>}
          <span className="text-fg-muted">{item.evidence_count} evidence</span> · Updated {formatDateTime(item.updated_at)}
        </p>
      </div>
    </div>
    <nav aria-label="Investigation sections" className="overflow-x-auto px-2 sm:px-4 lg:px-6">
      <ul className="flex min-w-max">
        {CASE_NAV.map((navItem) => {
          const active = navItem.section === section;
          return (
            <li key={navItem.section}>
              <Link
                to={caseHref(item.case_id, navItem.section)}
                aria-current={active ? 'page' : undefined}
                className={`relative flex h-10 items-center gap-2 px-3 font-mono text-micro uppercase transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-accent ${
                  active ? 'text-fg' : 'text-fg-subtle hover:text-fg'
                }`}
              >
                <navItem.icon aria-hidden="true" className={`h-3.5 w-3.5 ${active ? 'text-signal-accent' : ''}`} />
                {navItem.short}
                <span aria-hidden="true" className={`absolute inset-x-3 bottom-0 h-0.5 bg-signal-accent transition-opacity ${active ? 'opacity-100' : 'opacity-0'}`} />
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  </header>
);

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
