import React, { useId, useMemo, useState } from 'react';
import { AlertTriangle, ArrowUpRight, GitFork, ListOrdered } from 'lucide-react';
import { DEMO_CASE } from '../../../../fixtures/synthetic/case026';
import { SECTION_IDS } from '../../data/siteContent';
import { Container, ProvenanceLegend, Reveal } from '../../../../components/ui/primitives';
import { Link } from '../../../../router/Link';
import { ROUTES, caseHref } from '../../../../router/router';
import { EditorialHeading } from '../../board/materials';
import { EventTimeline } from './EventTimeline';
import { EvidencePanel } from './EvidencePanel';
import { KnowledgeGraph } from './KnowledgeGraph';
import { ProvenancePanel } from './ProvenancePanel';
import { countReviewItems, deriveSelection, type WorkbenchSelection } from './selection';

type View = 'graph' | 'timeline';

const VIEWS: { id: View; label: string; icon: typeof GitFork }[] = [
  { id: 'graph', label: 'Knowledge graph', icon: GitFork },
  { id: 'timeline', label: 'Event timeline', icon: ListOrdered },
];

const DISCREPANCY = DEMO_CASE.discrepancies[0];

/** Real workspace routes for the same synthetic case. */
const WORKSPACE_VIEWS = [
  { label: 'Dashboard', href: ROUTES.workspace },
  { label: 'Evidence', href: caseHref(DEMO_CASE.id, 'evidence') },
  { label: 'Knowledge graph', href: caseHref(DEMO_CASE.id, 'graph') },
  { label: 'Timeline', href: caseHref(DEMO_CASE.id, 'timeline') },
  { label: 'Spatial', href: caseHref(DEMO_CASE.id, 'spatial') },
  { label: 'Contradictions', href: caseHref(DEMO_CASE.id, 'contradictions') },
];

const OVERVIEW = [
  { label: 'Evidence', value: DEMO_CASE.evidence.length },
  { label: 'Extracted events', value: DEMO_CASE.events.length },
  { label: 'Entities', value: DEMO_CASE.entities.length },
  { label: 'Needs review', value: countReviewItems(), alert: true },
];

export const ProductPreview: React.FC = () => {
  const [selection, setSelection] = useState<WorkbenchSelection>({ kind: 'evidence', id: 'EV-01' });
  const [view, setView] = useState<View>('graph');
  const derived = useMemo(() => deriveSelection(selection), [selection]);
  const baseId = useId();

  const selectEvidence = (id: string): void => setSelection({ kind: 'evidence', id });

  const showDiscrepancyInTimeline = (): void => {
    const event = DEMO_CASE.events.find((item) => item.evidenceId === DISCREPANCY?.recordA.evidenceId);
    if (event) setSelection({ kind: 'event', id: event.id });
    setView('timeline');
  };

  return (
    <section id={SECTION_IDS.platform} aria-labelledby="workbench-heading" className="relative overflow-hidden border-b border-ink-line bg-[radial-gradient(ellipse_at_50%_0%,#1a1612,#080807_60%)] py-section">
      <div aria-hidden="true" className="pattern-dots pointer-events-none absolute inset-x-0 top-0 h-[36rem] opacity-30 [mask-image:linear-gradient(to_bottom,black,transparent)]" />
      <Container className="relative">
        <Reveal>
          <EditorialHeading
            id="workbench-heading"
            index="07"
            kicker="The YUKTI workspace"
            title="From the board"
            emphasis="to the case file."
            lead="The same evidence, in the investigator’s workspace. Click anything — every panel follows and shows where it came from."
          />
        </Reveal>

        <Reveal className="mt-14">
          <div className="screen-tilt border border-ink-strong bg-ink-950 shadow-[0_60px_140px_-40px_rgba(0,0,0,0.95),0_0_0_1px_rgba(216,197,167,0.06)]">
            {/* Window chrome */}
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-ink-line px-4 py-2.5">
              <div className="flex min-w-0 items-center gap-3 font-mono text-micro uppercase text-fg-subtle">
                <span aria-hidden="true" className="flex gap-1.5">
                  <span className="h-2 w-2 border border-ink-strong" />
                  <span className="h-2 w-2 border border-ink-strong" />
                </span>
                <span className="truncate">yukti / workspace / {DEMO_CASE.id.toLowerCase()}</span>
              </div>
              <span className="border border-signal-accent/50 bg-signal-accent/10 px-2 py-0.5 font-mono text-micro uppercase text-signal-accent">
                Synthetic demonstration data
              </span>
            </div>

            {/* Case overview */}
            <div className="grid border-b border-ink-line lg:grid-cols-12">
              <div className="border-b border-ink-line px-5 py-4 lg:col-span-5 lg:border-b-0 lg:border-r">
                <p className="font-mono text-micro uppercase text-fg-subtle">{DEMO_CASE.id}</p>
                <h3 className="mt-1 font-serif text-[1.75rem] font-normal leading-tight text-fg-bright">{DEMO_CASE.title}</h3>
                <p className="mt-2 inline-flex items-center gap-2 font-mono text-micro uppercase text-signal-accent">
                  <span aria-hidden="true" className="h-1.5 w-1.5 bg-signal-accent" />
                  {DEMO_CASE.status}
                </p>
              </div>
              <dl className="grid grid-cols-2 sm:grid-cols-4 lg:col-span-7">
                {OVERVIEW.map((item, index) => (
                  <div
                    key={item.label}
                    className={`border-ink-line px-5 py-4 ${index % 2 === 0 ? 'border-r' : 'sm:border-r'} ${index < 2 ? 'border-b sm:border-b-0' : ''} last:border-r-0`}
                  >
                    <dt className="font-mono text-micro uppercase text-fg-subtle">{item.label}</dt>
                    <dd className={`mt-1 font-display text-3xl font-semibold ${item.alert ? 'text-signal-flag' : 'text-fg'}`}>
                      {String(item.value).padStart(2, '0')}
                    </dd>
                  </div>
                ))}
              </dl>
            </div>

            {/* Workspace */}
            <div className="grid lg:grid-cols-12">
              <div className="border-b border-ink-line lg:col-span-3 lg:border-b-0 lg:border-r">
                <EvidencePanel
                  selectedId={selection.kind === 'evidence' ? selection.id : null}
                  related={derived.evidenceIds}
                  onSelect={selectEvidence}
                />
              </div>

              <div className="min-w-0 border-b border-ink-line lg:col-span-6 lg:border-b-0 lg:border-r">
                <div role="tablist" aria-label="Workspace view" className="flex border-b border-ink-line">
                  {VIEWS.map((item) => (
                    <button
                      key={item.id}
                      id={`${baseId}-${item.id}-tab`}
                      role="tab"
                      type="button"
                      aria-selected={view === item.id}
                      aria-controls={`${baseId}-view`}
                      onClick={() => setView(item.id)}
                      className={`flex items-center gap-2 border-r border-ink-line px-4 py-2.5 font-mono text-micro uppercase transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-accent ${
                        view === item.id ? 'bg-ink-900 text-fg' : 'text-fg-subtle hover:text-fg'
                      }`}
                    >
                      <item.icon aria-hidden="true" className="h-3.5 w-3.5" />
                      {item.label}
                    </button>
                  ))}
                </div>
                <div id={`${baseId}-view`} role="tabpanel" aria-labelledby={`${baseId}-${view}-tab`} className="p-3 sm:p-5">
                  {view === 'graph' ? (
                    <>
                      {/* Diagram keeps a legible minimum width and scrolls inside its panel on small screens. */}
                      <div className="-mx-3 overflow-x-auto px-3 sm:mx-0 sm:px-0">
                        <KnowledgeGraph
                          className="min-w-[520px]"
                          titleId={`${baseId}-graph-title`}
                          highlighted={derived.entityIds}
                          selectedEntityId={derived.selectedEntityId}
                          onSelectEntity={(entity) => setSelection({ kind: 'entity', id: entity.id })}
                        />
                      </div>
                      <p className="mt-2 font-mono text-micro uppercase text-fg-subtle sm:hidden">Scroll the graph sideways · tap a node</p>
                      <ProvenanceLegend className="mt-3" />
                    </>
                  ) : (
                    <EventTimeline
                      highlighted={derived.eventIds}
                      selectedEventId={derived.selectedEventId}
                      onSelectEvent={(event) => setSelection({ kind: 'event', id: event.id })}
                    />
                  )}
                </div>

                {DISCREPANCY && (
                  <div className="border-t border-ink-line bg-signal-flag/[0.04] px-5 py-4">
                    <div className="flex items-start gap-3">
                      <AlertTriangle aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-signal-flag" />
                      <div className="min-w-0 flex-1">
                        <p className="font-mono text-micro uppercase text-signal-flag">
                          Investigation insight · {DISCREPANCY.id}
                        </p>
                        <p className="mt-1.5 text-sm leading-relaxed text-fg">{DISCREPANCY.summary}</p>
                        <p className="mt-2 font-mono text-micro text-fg-subtle">
                          {DISCREPANCY.recordA.evidenceId} {DISCREPANCY.recordA.value} ↔ {DISCREPANCY.recordB.evidenceId}{' '}
                          {DISCREPANCY.recordB.value} · Δ {DISCREPANCY.variance}
                        </p>
                        <button
                          type="button"
                          onClick={showDiscrepancyInTimeline}
                          className="mt-3 border border-ink-strong px-3 py-1.5 font-mono text-micro uppercase text-fg transition-colors hover:border-fg-muted focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent"
                        >
                          Show in timeline
                        </button>
                      </div>
                    </div>
                  </div>
                )}
              </div>

              <div className="lg:col-span-3">
                <ProvenancePanel selection={selection} onSelectEvidence={selectEvidence} />
              </div>
            </div>
          </div>
          <p className="mt-4 font-mono text-micro uppercase text-fg-subtle">
            Front-end demonstration only · records are fictional and were not processed from uploaded evidence
          </p>
        </Reveal>

        <Reveal className="mt-14">
          <nav aria-label="Workspace views">
            <p className="font-mono text-[0.625rem] uppercase tracking-[0.16em] text-fg-subtle">Open a view in the workspace · {DEMO_CASE.id}</p>
            <ul className="mt-4 grid gap-px border border-ink-line bg-ink-line sm:grid-cols-2 lg:grid-cols-6">
              {WORKSPACE_VIEWS.map((view) => (
                <li key={view.label}>
                  <Link
                    to={view.href}
                    className="group flex h-full items-center justify-between gap-3 bg-ink-950 px-4 py-4 text-sm text-fg-muted transition-colors duration-base hover:bg-ink-900 hover:text-fg-bright focus:outline-none focus-visible:bg-ink-900 focus-visible:text-fg-bright"
                  >
                    {view.label}
                    <ArrowUpRight aria-hidden="true" className="h-4 w-4 text-fg-subtle transition-colors group-hover:text-crimson" />
                  </Link>
                </li>
              ))}
            </ul>
          </nav>
        </Reveal>
      </Container>
    </section>
  );
};
