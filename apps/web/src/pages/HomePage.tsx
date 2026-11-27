import React, { useMemo, useState } from 'react';
import { AlertTriangle, Archive, ArrowRight, MonitorPlay, Database, FolderOpen, FolderSearch, History, ListOrdered, Server, Share2, type LucideIcon } from 'lucide-react';
import { BackendStatusCard } from '../components/BackendStatusCard';
import { DEFAULT_HEALTH_POLL_INTERVAL_MS } from '../hooks/useBackendHealth';
import { Link } from '../router/Link';
import { caseHref } from '../router/router';
import { HEALTH_PATH } from '../services/api';
import { buildApiUrl } from '../services/config';
import type { CaseStatus } from '../types';
import { useSharedBackendHealth } from '../features/workspace/data/BackendHealthContext';
import { useCaseDetail, useIsSynthetic, useWorkspaceRepository } from '../features/workspace/data/WorkspaceDataContext';
import { EvidencePreview } from '../features/workspace/components/EvidencePreview';
import { CliCommand } from '../features/cli-bridge/CliCommand';
import { useWorkspaceShell } from '../features/workspace/layout/WorkspaceShellContext';
import {
  CaseStatusBadge,
  EmptyState,
  ErrorState,
  FilterChip,
  LoadingState,
  Panel,
  SearchField,
  SelectField,
  SyntheticTag,
} from '../features/workspace/components/ui';
import { CASE_STATUS_LABEL, formatDate, formatDateTime } from '../features/workspace/format';
import type { WorkspaceCase } from '../features/workspace/types';

/** Configured health URL for display; never throws during render. */
function describeHealthUrl(): string {
  try {
    return buildApiUrl(HEALTH_PATH);
  } catch {
    return 'an invalid VITE_API_URL';
  }
}

const STATUSES: CaseStatus[] = ['ACTIVE', 'UNDER_REVIEW', 'DRAFT', 'ARCHIVED'];

const STATUS_BAR: Record<CaseStatus, string> = {
  ACTIVE: 'bg-signal-ok',
  UNDER_REVIEW: 'bg-signal-accent',
  DRAFT: 'bg-fg-muted',
  ARCHIVED: 'bg-ink-strong',
};

type SortKey = 'updated' | 'case' | 'evidence' | 'review';

const SORTERS: Record<SortKey, (a: WorkspaceCase, b: WorkspaceCase) => number> = {
  updated: (a, b) => b.updated_at.localeCompare(a.updated_at),
  case: (a, b) => a.case_id.localeCompare(b.case_id),
  evidence: (a, b) => b.evidence_count - a.evidence_count,
  review: (a, b) => b.review_count - a.review_count,
};

/* ------------------------------------------------------------------ Metrics */

const MetricsBand: React.FC<{ cases: WorkspaceCase[]; synthetic: boolean }> = ({ cases, synthetic }) => {
  const sum = (key: 'evidence_count' | 'event_count' | 'entity_count' | 'review_count'): number =>
    cases.reduce((total, c) => total + c[key], 0);
  const metrics: { label: string; value: number; icon: LucideIcon; alert?: boolean }[] = [
    { label: 'Total cases', value: cases.length, icon: FolderOpen },
    { label: 'Evidence files', value: sum('evidence_count'), icon: Archive },
    { label: 'Events', value: sum('event_count'), icon: ListOrdered },
    { label: 'Entities', value: sum('entity_count'), icon: Share2 },
    { label: 'Needs review', value: sum('review_count'), icon: AlertTriangle, alert: true },
  ];
  return (
    <section aria-labelledby="metrics-heading" className="border-b border-ink-line">
      <div className="flex flex-wrap items-center justify-between gap-2 px-4 pt-4 sm:px-6 lg:px-8">
        <h2 id="metrics-heading" className="font-mono text-micro uppercase text-fg-subtle">
          Case portfolio
        </h2>
        {synthetic && <SyntheticTag />}
      </div>
      <div className="px-4 pt-3 sm:px-6 lg:px-8">
        <div className="flex h-1.5 w-full gap-px bg-ink-950" role="img" aria-label={`Case status: ${STATUSES.map((s) => `${cases.filter((c) => c.status === s).length} ${CASE_STATUS_LABEL[s]}`).join(', ')}`}>
          {STATUSES.map((s) => {
            const count = cases.filter((c) => c.status === s).length;
            return count > 0 ? <span key={s} className={STATUS_BAR[s]} style={{ flexGrow: count }} title={`${CASE_STATUS_LABEL[s]}: ${count}`} /> : null;
          })}
        </div>
        <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-1 font-mono text-micro uppercase text-fg-subtle" aria-hidden="true">
          {STATUSES.map((s) => (
            <li key={s} className="flex items-center gap-1.5"><span className={`h-1.5 w-1.5 ${STATUS_BAR[s]}`} />{CASE_STATUS_LABEL[s]} {cases.filter((c) => c.status === s).length}</li>
          ))}
        </ul>
      </div>
      <dl className="mt-4 grid grid-cols-2 border-t border-ink-line sm:grid-cols-5">
        {metrics.map((metric) => (
          <div key={metric.label} className="flex items-center gap-3 border-ink-line px-4 py-4 sm:px-6 lg:px-8 [&:not(:last-child)]:border-r max-sm:[&:nth-child(2n)]:border-r-0">
            <metric.icon aria-hidden="true" strokeWidth={1.4} className={`h-5 w-5 shrink-0 ${metric.alert ? 'text-crimson' : 'text-fg-subtle'}`} />
            <div className="flex flex-col-reverse">
              <dt className="font-mono text-micro uppercase text-fg-subtle">{metric.label}</dt>
              <dd className={`font-display text-3xl font-semibold leading-none ${metric.alert ? 'text-signal-flag' : 'text-fg-bright'}`}>{metric.value}</dd>
            </div>
          </div>
        ))}
      </dl>
    </section>
  );
};

/* --------------------------------------------------------------- Case table */

const CaseActions: React.FC<{ item: WorkspaceCase }> = ({ item }) => (
  <div className="flex items-center gap-3 font-mono text-micro uppercase">
    <Link to={caseHref(item.case_id)} className="inline-flex items-center gap-1 text-fg hover:text-signal-accent">
      Open <ArrowRight aria-hidden="true" className="h-3 w-3" />
      <span className="sr-only">{item.case_id}</span>
    </Link>
    {item.has_detail && (
      <>
        <Link to={caseHref(item.case_id, 'evidence')} className="text-fg-muted hover:text-fg">
          Evidence<span className="sr-only"> for {item.case_id}</span>
        </Link>
        <Link to={caseHref(item.case_id, 'timeline')} className="hidden text-fg-muted hover:text-fg 2xl:inline">
          Timeline<span className="sr-only"> for {item.case_id}</span>
        </Link>
      </>
    )}
  </div>
);

const CaseTable: React.FC<{ cases: WorkspaceCase[] }> = ({ cases }) => {
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState<CaseStatus | 'ALL'>('ALL');
  const [sort, setSort] = useState<SortKey>('updated');

  const counts = useMemo(() => {
    const result = { ALL: cases.length } as Record<CaseStatus | 'ALL', number>;
    for (const s of STATUSES) result[s] = cases.filter((c) => c.status === s).length;
    return result;
  }, [cases]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return cases
      .filter((c) => status === 'ALL' || c.status === status)
      .filter((c) => !q || [c.case_id, c.title, c.fir_number ?? ''].some((field) => field.toLowerCase().includes(q)))
      .sort(SORTERS[sort]);
  }, [cases, query, status, sort]);
  const maxEvidence = Math.max(1, ...cases.map((c) => c.evidence_count));

  return (
    <Panel title="Investigations" meta={`${rows.length} of ${cases.length}`} labelledBy="cases-heading">
      <div className="flex flex-col gap-3 border-b border-ink-line p-3 sm:p-4">
        <div className="flex flex-col gap-2 sm:flex-row">
          <SearchField value={query} onChange={setQuery} label="Search cases" placeholder="Case ID, title or FIR" className="flex-1" />
          <SelectField
            label="Sort"
            value={sort}
            onChange={(value) => setSort(value as SortKey)}
            options={[
              { value: 'updated', label: 'Last updated' },
              { value: 'case', label: 'Case ID' },
              { value: 'evidence', label: 'Evidence count' },
              { value: 'review', label: 'Needs review' },
            ]}
            className="sm:w-56"
          />
        </div>
        <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filter by case status">
          <FilterChip active={status === 'ALL'} onClick={() => setStatus('ALL')} count={counts.ALL}>
            All
          </FilterChip>
          {STATUSES.map((s) => (
            <FilterChip key={s} active={status === s} onClick={() => setStatus(s)} count={counts[s]}>
              {CASE_STATUS_LABEL[s]}
            </FilterChip>
          ))}
        </div>
      </div>

      {rows.length === 0 ? (
        <EmptyState icon={FolderSearch} title="No matching cases">
          Adjust the search or status filter.
        </EmptyState>
      ) : (
        <>
          <table className="hidden w-full text-left md:table">
            <thead>
              <tr className="border-b border-ink-line font-mono text-micro uppercase text-fg-subtle">
                <th scope="col" className="px-4 py-2.5 font-normal">Case</th>
                <th scope="col" className="px-4 py-2.5 font-normal">Status</th>
                <th scope="col" className="px-4 py-2.5 text-right font-normal">Evidence</th>
                <th scope="col" className="px-4 py-2.5 text-right font-normal">Review</th>
                <th scope="col" className="hidden px-4 py-2.5 font-normal lg:table-cell">Updated</th>
                <th scope="col" className="px-4 py-2.5 font-normal"><span className="sr-only">Actions</span></th>
              </tr>
            </thead>
            <tbody className="divide-y divide-ink-line">
              {rows.map((item) => (
                <tr key={item.case_id} className="group transition-colors hover:bg-white/[0.02]">
                  <td className="px-4 py-3">
                    <Link to={caseHref(item.case_id)} className="block focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent">
                      <span className="font-mono text-micro text-fg-subtle">{item.case_id}</span>
                      <span className="block text-sm font-medium text-fg group-hover:text-signal-accent">{item.title}</span>
                    </Link>
                  </td>
                  <td className="px-4 py-3"><CaseStatusBadge status={item.status} /></td>
                  <td className="px-4 py-3 text-right font-mono text-sm text-fg">
                    <span className="inline-flex items-center gap-2">
                      <span aria-hidden="true" className="hidden h-1 w-14 bg-ink-700 lg:block">
                        <span className="block h-full bg-fg/70" style={{ width: `${(item.evidence_count / maxEvidence) * 100}%` }} />
                      </span>
                      {item.evidence_count}
                    </span>
                  </td>
                  <td className={`px-4 py-3 text-right font-mono text-sm ${item.review_count > 0 ? 'text-signal-flag' : 'text-fg-subtle'}`}>
                    {item.review_count > 0 ? item.review_count : '—'}
                    <span className="sr-only">{item.review_count > 0 ? ' items need review' : ' no open review items'}</span>
                  </td>
                  <td className="hidden px-4 py-3 font-mono text-xs text-fg-muted lg:table-cell">{formatDate(item.updated_at)}</td>
                  <td className="px-4 py-3"><CaseActions item={item} /></td>
                </tr>
              ))}
            </tbody>
          </table>
          <ul className="divide-y divide-ink-line md:hidden">
            {rows.map((item) => (
              <li key={item.case_id} className="flex flex-col gap-2 px-4 py-3">
                <div className="flex items-start justify-between gap-3">
                  <Link to={caseHref(item.case_id)} className="min-w-0">
                    <span className="font-mono text-micro text-fg-subtle">{item.case_id}</span>
                    <span className="block text-sm font-medium text-fg">{item.title}</span>
                  </Link>
                  <CaseStatusBadge status={item.status} />
                </div>
                <p className="font-mono text-micro uppercase text-fg-subtle">
                  {item.evidence_count} evidence · {item.review_count} to review · {formatDate(item.updated_at)}
                </p>
                <CaseActions item={item} />
              </li>
            ))}
          </ul>
        </>
      )}
    </Panel>
  );
};

/* ------------------------------------------------------------ Side panels */

const RecentActivity: React.FC<{ cases: WorkspaceCase[]; synthetic: boolean }> = ({ cases, synthetic }) => {
  const items = [...cases]
    .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
    .slice(0, 5)
    .map((c) => ({
      id: c.case_id,
      when: c.updated_at,
      text: c.created_at === c.updated_at ? 'Case record created' : 'Case record updated',
      title: c.title,
    }));
  return (
    <Panel title="Recent activity" meta={synthetic ? 'synthetic · case timestamps' : undefined}>
      {items.length === 0 ? (
        <EmptyState icon={History} title="No activity yet" />
      ) : (
        <ol className="divide-y divide-ink-line">
          {items.map((item) => (
            <li key={item.id} className="grid grid-cols-[auto_1fr] gap-x-3 px-4 py-3">
              <span aria-hidden="true" className="mt-1.5 h-1.5 w-1.5 bg-fg-subtle" />
              <div className="min-w-0">
                <p className="text-sm text-fg">
                  {item.text} · <Link to={caseHref(item.id)} className="font-mono text-xs text-fg-muted hover:text-signal-accent">{item.id}</Link>
                </p>
                <p className="truncate text-xs text-fg-subtle">{item.title}</p>
                <time dateTime={item.when} className="mt-0.5 block font-mono text-micro text-fg-subtle">
                  {formatDateTime(item.when)}
                </time>
              </div>
            </li>
          ))}
        </ol>
      )}
      <p className="border-t border-ink-line px-4 py-2.5 font-mono text-micro uppercase text-fg-subtle">From case timestamps · no audit-log API yet</p>
    </Panel>
  );
};

const DataSourcesPanel: React.FC<{ synthetic: boolean }> = ({ synthetic }) => (
  <Panel title="Data sources">
    <ul className="divide-y divide-ink-line text-sm">
      <li className="flex items-center justify-between gap-3 px-4 py-3">
        <span className="flex items-center gap-2 text-fg"><Server aria-hidden="true" className="h-4 w-4 text-signal-ok" /> Backend API</span>
        <span className="font-mono text-micro uppercase text-signal-ok">Live · /health</span>
      </li>
      <li className="flex items-center justify-between gap-3 px-4 py-3">
        <span className="flex items-center gap-2 text-fg">
          <Database aria-hidden="true" className={`h-4 w-4 ${synthetic ? 'text-signal-accent' : 'text-signal-ok'}`} /> Case records
        </span>
        <span className={`font-mono text-micro uppercase ${synthetic ? 'text-signal-accent' : 'text-signal-ok'}`}>{synthetic ? 'Synthetic fixture' : 'API'}</span>
      </li>
      {synthetic && (
        <li className="flex flex-wrap items-center gap-1.5 px-4 py-3" aria-label="Not available in this build">
          <span className="mr-1 font-mono text-micro uppercase text-fg-subtle">Not yet available</span>
          {['Case creation', 'Ingestion', 'Case APIs'].map((item) => (
            <span key={item} className="border border-dashed border-ink-strong px-1.5 py-0.5 font-mono text-micro uppercase text-fg-subtle">{item}</span>
          ))}
        </li>
      )}
    </ul>
  </Panel>
);

/* ---------------------------------------------------------- Featured case */

/** The investigation in focus, shown through its evidence rather than described. */
const FeaturedCase: React.FC<{ caseId: string }> = ({ caseId }) => {
  const detail = useCaseDetail(caseId);
  const synthetic = useIsSynthetic(detail.state.data?.case);
  if (detail.state.status === 'loading') {
    return (
      <Panel title="In focus">
        <LoadingState label={`Loading ${caseId}`} />
      </Panel>
    );
  }
  if (detail.state.status === 'error') {
    return (
      <Panel title="In focus">
        <ErrorState message={detail.state.error} onRetry={detail.retry} />
      </Panel>
    );
  }
  const data = detail.state.data;
  if (!data) return null;
  const open = data.review_items.filter((item) => item.status !== 'verified');
  const conflicts = open.filter((item) => item.category === 'conflicting').length;
  return (
    <Panel title="In focus" meta={data.case.case_id} actions={<CaseStatusBadge status={data.case.status} />}>
      <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-end sm:justify-between">
        <div className="min-w-0">
          <Link to={caseHref(caseId)} className="font-serif text-2xl leading-tight text-fg-bright hover:text-paper">
            {data.case.title}
          </Link>
          <p className="mt-1 font-mono text-micro uppercase text-fg-subtle">
            {data.evidence.length} evidence · {data.events.length} events · {data.entities.length} entities
          </p>
        </div>
        <Link
          to={caseHref(caseId, 'contradictions')}
          className={`inline-flex shrink-0 items-center gap-2 border px-3 py-2 font-mono text-micro uppercase transition-colors ${conflicts > 0 ? 'border-crimson/60 text-crimson hover:bg-crimson/10' : 'border-ink-line text-fg-muted hover:text-fg'}`}
        >
          <AlertTriangle aria-hidden="true" className="h-3.5 w-3.5" />
          {open.length} to review{conflicts > 0 ? ` · ${conflicts} conflicting` : ''}
        </Link>
      </div>
      <ul aria-label={`Evidence in ${caseId}`} className="grid grid-cols-3 gap-2 px-4 pb-4 sm:grid-cols-5">
        {data.evidence.map((evidence) => (
          <li key={evidence.evidence_id}>
            <Link to={caseHref(caseId, 'evidence', { evidence: evidence.evidence_id })} className="group block border border-ink-line transition-[border-color,transform] duration-base hover:border-ink-strong motion-safe:hover:-translate-y-0.5">
              <EvidencePreview evidence={evidence} synthetic={synthetic} />
              <span className={`block border-t border-ink-line px-2 py-1 font-mono text-micro ${evidence.review === 'flagged' ? 'text-crimson' : 'text-fg-subtle'}`}>{evidence.evidence_id}</span>
            </Link>
          </li>
        ))}
      </ul>
    </Panel>
  );
};

/* -------------------------------------------------------------------- Page */

/** Opening band: what YUKTI does, the one-click replay, and the link to the terminal. */
const WorkspaceHero: React.FC<{ caseId: string | null; live: boolean; liveCount: number }> = ({ caseId, live, liveCount }) => (
  <header className="hero-band relative overflow-hidden border-b border-ink-line">
    <div aria-hidden="true" className="hero-orbit pointer-events-none absolute -right-24 -top-24 h-[26rem] w-[26rem] rounded-full border border-crimson/15" />
    <div aria-hidden="true" className="hero-orbit hero-orbit--slow pointer-events-none absolute -right-8 -top-8 h-[18rem] w-[18rem] rounded-full border border-dashed border-paper/10" />
    <div className="relative grid gap-8 px-4 py-8 sm:px-6 lg:grid-cols-[minmax(0,1fr)_25rem] lg:px-8 lg:py-10">
      <div className="min-w-0">
        <p className="ws-rise text-sm font-medium text-crimson">Investigation workspace</p>
        <h1 className="ws-rise mt-3 font-serif text-[clamp(2.2rem,4.4vw,3.8rem)] font-semibold leading-[1.02] text-fg-bright" style={{ ['--i' as string]: 1 }}>
          Every source. One clock.
          <br />
          <span className="text-crimson">One story</span> you can defend.
        </h1>
        <p className="ws-rise mt-4 max-w-xl text-[0.95rem] leading-relaxed text-fg-muted" style={{ ['--i' as string]: 2 }}>
          YUKTI seals each exhibit, puts every record on one corrected clock, maps it, links it, and flags what does not add up, keeping
          what was observed apart from what was inferred.
        </p>
        {caseId && (
          <div className="ws-rise mt-6 flex flex-wrap gap-2" style={{ ['--i' as string]: 3 }}>
            <Link
              to={caseHref(caseId, 'theatre')}
              className="theatre-cta inline-flex h-11 items-center gap-2.5 rounded-full bg-crimson px-5 text-sm font-semibold text-white hover:bg-crimson-deep focus:outline-none focus-visible:ring-2 focus-visible:ring-white"
            >
              <MonitorPlay aria-hidden="true" className="h-4 w-4" /> Play the reconstruction
            </Link>
            <Link
              to={caseHref(caseId)}
              className="inline-flex h-11 items-center gap-2 rounded-full border border-white/15 bg-white/[0.04] px-5 text-sm font-medium text-fg hover:border-white/30 focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-accent"
            >
              Open {caseId} <ArrowRight aria-hidden="true" className="h-3.5 w-3.5" />
            </Link>
          </div>
        )}
      </div>
      <aside aria-label="Terminal link" className="ws-rise self-end overflow-hidden rounded-2xl border border-white/10 bg-black/40 backdrop-blur-md" style={{ ['--i' as string]: 4 }}>
        <div className="flex items-center gap-2 border-b border-ink-line px-3 py-2">
          <span className="flex gap-1" aria-hidden="true">
            <span className="h-2.5 w-2.5 rounded-full bg-crimson" />
            <span className="h-2.5 w-2.5 rounded-full bg-ink-strong" />
            <span className="h-2.5 w-2.5 rounded-full bg-ink-strong" />
          </span>
          <span className="flex-1 font-mono text-xs text-fg-muted">yukti cli</span>
          <span className={`flex items-center gap-1.5 text-xs font-medium ${live ? 'text-signal-ok' : 'text-signal-warn'}`}>
            <span className={`h-1.5 w-1.5 rounded-full ${live ? 'bg-signal-ok bridge-pulse' : 'bg-signal-warn'}`} aria-hidden="true" />
            {live ? 'linked' : 'not linked'}
          </span>
        </div>
        <div className="space-y-2.5 p-3">
          <p className="text-xs leading-relaxed text-fg-muted">
            {live
              ? `Web and terminal share one engine. ${liveCount} live ${liveCount === 1 ? 'case' : 'cases'} on it; new ones from the CLI appear here as they are created.`
              : 'Start the local engine from the terminal; this page links itself as soon as it answers.'}
          </p>
          <CliCommand command="yukti demo" compact />
          <CliCommand command={live ? 'yukti open --view theatre' : 'yukti server start'} compact />
        </div>
      </aside>
    </div>
  </header>
);

/** Workspace home (`/workspace`): case portfolio, investigations, connectivity and activity. */
export const HomePage: React.FC = () => {
  const health = useSharedBackendHealth();
  const repository = useWorkspaceRepository();
  const { cases } = useWorkspaceShell();
  const synthetic = repository.source === 'synthetic';

  return (
    <div>
      <WorkspaceHero
        caseId={repository.defaultCaseId}
        live={!synthetic}
        liveCount={cases.state.status === 'success' ? cases.state.data.filter((c) => c.source === 'api').length : 0}
      />

      {cases.state.status === 'success' && <MetricsBand cases={cases.state.data} synthetic={synthetic} />}

      <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
        <div className="min-w-0 space-y-4 xl:col-span-8">
          {repository.defaultCaseId && cases.state.status === 'success' && cases.state.data.length > 0 && (
            <FeaturedCase caseId={repository.defaultCaseId} />
          )}
          {cases.state.status === 'loading' && (
            <Panel title="Investigations">
              <LoadingState label="Loading cases" />
            </Panel>
          )}
          {cases.state.status === 'error' && (
            <Panel title="Investigations">
              <ErrorState message={cases.state.error} onRetry={cases.retry} />
            </Panel>
          )}
          {cases.state.status === 'success' &&
            (cases.state.data.length === 0 ? (
              <Panel title="Investigations">
                <EmptyState icon={FolderSearch} title="No cases yet">
                  Cases will appear here once the case API is available.
                </EmptyState>
              </Panel>
            ) : (
              <CaseTable cases={cases.state.data} />
            ))}
        </div>
        <div className="flex min-w-0 flex-col gap-4 xl:col-span-4">
          <BackendStatusCard health={health} healthUrl={describeHealthUrl()} pollIntervalMs={DEFAULT_HEALTH_POLL_INTERVAL_MS} />
          <DataSourcesPanel synthetic={synthetic} />
          {cases.state.status === 'success' && <RecentActivity cases={cases.state.data} synthetic={synthetic} />}
        </div>
      </div>
    </div>
  );
};
