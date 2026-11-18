import React, { useId, useMemo, useState } from 'react';
import { AlertTriangle, GitFork, ListOrdered } from 'lucide-react';
import { DEMO_CASE } from '../../../../fixtures/synthetic/case026';
import { SECTION_IDS } from '../../data/siteContent';
import { Container, Eyebrow, ProvenanceLegend, Reveal } from '../../../../components/ui/primitives';
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
    <section id={SECTION_IDS.workbench} aria-labelledby="workbench-heading" className="relative border-b border-ink-line py-section">
      <div aria-hidden="true" className="pattern-dots pointer-events-none absolute inset-x-0 top-0 h-[36rem] opacity-30 [mask-image:linear-gradient(to_bottom,black,transparent)]" />
      <Container className="relative">
        <Reveal className="grid gap-8 lg:grid-cols-12 lg:items-end">
          <div className="lg:col-span-8">
            <Eyebrow className="mb-5">03 — Evidence intelligence</Eyebrow>
            <h2 id="workbench-heading" className="font-display text-display-lg uppercase text-fg">
              <span className="block font-light">The investigation</span>{' '}
              <span className="block font-bold">workbench.</span>
            </h2>
          </div>
          <p className="text-base leading-relaxed text-fg-muted lg:col-span-4">
Click anything. Every panel follows — and shows where it came from.
          </p>
        </Reveal>

        <Reveal className="mt-12">
          <div className="border border-ink-strong bg-ink-950 shadow-[0_40px_120px_-40px_rgba(0,0,0,0.9)]">
            {/* Window chrome */}
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-ink-line px-4 py-2.5">
              <div className="flex min-w-0 items-center gap-3 font-mono text-micro uppercase text-fg-subtle">
                <span aria-hidden="true" className="flex gap-1.5">
                  <span className="h-2 w-2 border border-ink-strong" />
                  <span className="h-2 w-2 border border-ink-strong" />
                </span>
                <span className="truncate">yukti / workspace / {DEMO_CASE.id.toLowerCase()}</span>
              </div>
              <span className="border border-signal-amber/50 bg-signal-amber/10 px-2 py-0.5 font-mono text-micro uppercase text-signal-amber">
                Synthetic demonstration data
              </span>
            </div>

            {/* Case overview */}
            <div className="grid border-b border-ink-line lg:grid-cols-12">
              <div className="border-b border-ink-line px-5 py-4 lg:col-span-5 lg:border-b-0 lg:border-r">
                <p className="font-mono text-micro uppercase text-fg-subtle">{DEMO_CASE.id}</p>
                <h3 className="mt-1 font-display text-2xl font-semibold uppercase leading-tight text-fg">{DEMO_CASE.title}</h3>
                <p className="mt-2 inline-flex items-center gap-2 font-mono text-micro uppercase text-signal-amber">
                  <span aria-hidden="true" className="h-1.5 w-1.5 bg-signal-amber" />
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
                      className={`flex items-center gap-2 border-r border-ink-line px-4 py-2.5 font-mono text-micro uppercase transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-amber ${
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
                          className="mt-3 border border-ink-strong px-3 py-1.5 font-mono text-micro uppercase text-fg transition-colors hover:border-fg-muted focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-amber"
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
      </Container>
    </section>
  );
};
