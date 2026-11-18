import React, { useState } from 'react';
import { ArrowRight, Check, Lock } from 'lucide-react';
import { DEMO_CASE } from '../../../fixtures/synthetic/case026';
import { SECTION_IDS, WORKFLOW_STEPS, type WorkflowStep } from '../data/siteContent';
import { Container, Frame, ProvenanceBadge, Reveal, SectionHeader } from '../../../components/ui/primitives';

const IngestStage: React.FC = () => (
  <ul className="divide-y divide-ink-line font-mono text-xs">
    {DEMO_CASE.evidence.map((item) => (
      <li key={item.id} className="flex items-center gap-3 py-2.5">
        <Lock aria-hidden="true" className="h-3.5 w-3.5 shrink-0 text-fg-subtle" />
        <span className="min-w-0 flex-1 truncate text-fg">{item.fileName}</span>
        <span className="hidden text-fg-subtle sm:inline">sha256 {item.sha256}</span>
        <Check aria-label="Digest recorded" className="h-3.5 w-3.5 shrink-0 text-signal-ok" />
      </li>
    ))}
  </ul>
);

const Token: React.FC<{ tag: string; children: React.ReactNode }> = ({ tag, children }) => (
  <span className="relative mx-0.5 inline-flex flex-col items-start align-top">
    <span className="border-b border-signal-amber bg-signal-amber/10 px-1 text-fg">{children}</span>
    <span className="mt-1 font-mono text-[0.625rem] uppercase tracking-[0.06em] text-signal-amber">{tag}</span>
  </span>
);

const ExtractStage: React.FC = () => (
  <div>
    <p className="font-mono text-micro uppercase text-fg-subtle">EV-02 · door_controller_export.csv · row 3,118</p>
    <p className="mt-5 text-base leading-[2.6] text-fg-muted">
      <Token tag="credential">Badge 4471</Token> granted at <Token tag="location">Gate B</Token> on{' '}
      <Token tag="timestamp">2026-03-14 22:43:52</Token> by controller <Token tag="device">DC-07</Token>
    </p>
    <p className="mt-4 font-mono text-micro uppercase text-fg-subtle">4 values extracted · each keeps its row reference</p>
  </div>
);

const CorrelateStage: React.FC = () => (
  <div className="grid gap-3 sm:grid-cols-[1fr_auto_1fr] sm:items-center">
    {[
      ['EV-02 · access log', 'Badge 4471 · Gate B · 22:43:52'],
      ['EV-05 · network log', 'D-01 · AP-WH7 · 22:44:10'],
    ].map(([title, body], index) => (
      <React.Fragment key={title}>
        {index === 1 && (
          <div className="flex flex-col items-center gap-2 py-2">
            <svg aria-hidden="true" width="64" height="6" viewBox="0 0 64 6" className="rotate-90 sm:rotate-0">
              <line x1="0" y1="3" x2="64" y2="3" stroke="#FFB224" strokeWidth="1.5" strokeDasharray="5 5" className="flow-line" />
            </svg>
            <span className="font-mono text-micro uppercase text-signal-amber">same place · 18 s apart</span>
          </div>
        )}
        <div className="border border-ink-line bg-ink-950 p-4">
          <p className="font-mono text-micro uppercase text-fg-subtle">{title}</p>
          <p className="mt-2 font-mono text-xs text-fg">{body}</p>
        </div>
      </React.Fragment>
    ))}
    <div className="flex flex-wrap items-center gap-2 sm:col-span-3">
      <ProvenanceBadge value="inferred" />
      <span className="text-sm text-fg-muted">Co-presence link proposed — stored with both supporting rows.</span>
    </div>
  </div>
);

const ReconstructStage: React.FC = () => (
  <ol className="relative space-y-3 border-l border-ink-strong pl-5">
    {DEMO_CASE.events.map((event) => {
      const flagged = event.evidenceId === 'EV-01' || event.evidenceId === 'EV-02';
      return (
        <li key={event.id} className="relative">
          <span
            aria-hidden="true"
            className={`absolute -left-[1.6rem] top-1.5 h-2 w-2 rotate-45 ${flagged ? 'bg-signal-flag' : 'bg-fg'}`}
          />
          <p className="flex flex-wrap items-baseline gap-x-3 text-sm">
            <span className="font-mono text-xs text-fg-muted">{event.time}</span>
            <span className="text-fg">{event.title}</span>
            {flagged && <span className="font-mono text-micro uppercase text-signal-flag">order uncertain</span>}
          </p>
        </li>
      );
    })}
  </ol>
);

const STAGES: Record<WorkflowStep['id'], React.FC> = {
  ingest: IngestStage,
  extract: ExtractStage,
  correlate: CorrelateStage,
  reconstruct: ReconstructStage,
};

export const WorkflowSection: React.FC = () => {
  const [activeIndex, setActiveIndex] = useState(0);
  const active = WORKFLOW_STEPS[activeIndex] ?? WORKFLOW_STEPS[0]!;
  const Stage = STAGES[active.id];
  // Markers sit at the start of each equal column, so the fill reaches index × column width.
  const progress = (activeIndex / WORKFLOW_STEPS.length) * 100;
  const next = WORKFLOW_STEPS[activeIndex + 1];

  return (
    <section id={SECTION_IDS.workflow} aria-labelledby="workflow-heading" className="border-b border-ink-line py-section">
      <Container>
        <SectionHeader
          id="workflow-heading"
          eyebrow="04 — How YUKTI works"
          lead="From raw evidence"
          emphasis="to a reconstructed investigation."
          aside={<p>Four stages. Each keeps a reference to its artifact.</p>}
        />

        <Reveal className="mt-14">
          {/* Stage track */}
          <div className="relative">
            <div aria-hidden="true" className="absolute left-0 right-0 top-[1.0625rem] hidden h-px bg-ink-strong md:block" />
            <div
              aria-hidden="true"
              className="absolute left-0 top-[1.0625rem] hidden h-px bg-signal-amber transition-[width] duration-slow ease-out md:block"
              style={{ width: `${progress}%` }}
            />
            <ol className="relative grid gap-px bg-ink-line md:grid-cols-4 md:gap-0 md:bg-transparent">
              {WORKFLOW_STEPS.map((step, index) => {
                const selected = index === activeIndex;
                const done = index < activeIndex;
                return (
                  <li key={step.id} className="bg-ink-950 md:bg-transparent">
                    <button
                      type="button"
                      aria-current={selected ? 'step' : undefined}
                      onClick={() => setActiveIndex(index)}
                      className="group flex w-full items-start gap-4 py-4 text-left focus:outline-none md:flex-col md:gap-5 md:py-0 md:pr-6"
                    >
                      <span
                        className={`relative z-10 flex h-[2.125rem] w-[2.125rem] shrink-0 items-center justify-center border font-mono text-xs transition-colors duration-base group-focus-visible:ring-2 group-focus-visible:ring-signal-amber ${
                          selected
                            ? 'border-signal-amber bg-signal-amber text-ink-950'
                            : done
                              ? 'border-signal-amber bg-ink-950 text-signal-amber'
                              : 'border-ink-strong bg-ink-950 text-fg-subtle group-hover:border-fg-muted group-hover:text-fg'
                        }`}
                      >
                        {step.index}
                      </span>
                      <span>
                        <span
                          className={`block font-display text-3xl font-bold uppercase leading-none transition-colors duration-base ${
                            selected ? 'text-fg' : 'text-fg-subtle group-hover:text-fg-muted'
                          }`}
                        >
                          {step.title}
                        </span>
                        <span className="mt-2 block max-w-[16rem] text-sm leading-relaxed text-fg-muted">{step.description}</span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ol>
          </div>

          {/* Stage detail */}
          <div className="mt-10 grid gap-px border border-ink-line bg-ink-line lg:grid-cols-12">
            <div className="flex flex-col justify-between gap-8 bg-ink-950 p-6 sm:p-8 lg:col-span-4">
              <div>
                <p className="font-mono text-micro uppercase text-signal-amber">
                  Stage {active.index} / {String(WORKFLOW_STEPS.length).padStart(2, '0')}
                </p>
                <p className="mt-4 font-display text-display-md font-bold uppercase text-fg">{active.title}</p>
                <p className="mt-4 text-sm leading-relaxed text-fg-muted">{active.detail}</p>
              </div>
              {next ? (
                <button
                  type="button"
                  onClick={() => setActiveIndex(activeIndex + 1)}
                  className="group inline-flex items-center gap-2 self-start font-mono text-micro uppercase text-fg transition-colors hover:text-signal-amber focus:outline-none focus-visible:text-signal-amber"
                >
                  Next: {next.title}
                  <ArrowRight aria-hidden="true" className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => setActiveIndex(0)}
                  className="self-start font-mono text-micro uppercase text-fg-muted transition-colors hover:text-fg focus:outline-none focus-visible:text-signal-amber"
                >
                  ↺ Restart from ingest
                </button>
              )}
            </div>
            <div className="bg-ink-900 p-4 sm:p-8 lg:col-span-8">
              <Frame title={<span>{active.title} · sample</span>} meta="Synthetic" bodyClassName="p-4 sm:p-6 min-h-[15rem]">
                <div key={active.id} aria-live="polite" className="animate-[fadeIn_400ms_cubic-bezier(0.22,1,0.36,1)]">
                  <Stage />
                </div>
              </Frame>
            </div>
          </div>
        </Reveal>
      </Container>
    </section>
  );
};
