import React from 'react';
import { DELIVERABLES, DELIVERABLE_STATUS_LABEL, SECTION_IDS, type DeliverableStatus } from '../data/siteContent';
import { EditorialHeading, Pushpin } from '../board/materials';
import { Reveal } from '../../../components/ui/primitives';

const PIPELINE = [
  { index: '01', title: 'Ingest', line: 'Originals stay read-only; SHA-256 recorded at intake.' },
  { index: '02', title: 'Extract', line: 'Entities, times and places — each value keeps its artifact reference.' },
  { index: '03', title: 'Correlate', line: 'Links proposed across sources, labelled inferred until reviewed.' },
  { index: '04', title: 'Reconstruct', line: 'Timeline and spatial views; conflicts and uncertainty go to review.' },
  { index: '05', title: 'Report', line: 'Findings exported with the provenance that supports them.' },
];

const STACK = [
  { layer: 'Interface', tech: 'React · TypeScript · Vite' },
  { layer: 'Services', tech: 'Python 3.12 · FastAPI' },
  { layer: 'Data', tech: 'PostgreSQL 16 · Neo4j 5', note: 'persistence planned' },
  { layer: 'Decision support', tech: 'Node.js 22 · TypeScript', note: 'local inference — future' },
];

const STAMP: Record<DeliverableStatus, string> = {
  implemented: 'border-[#2f6b45] text-[#2f6b45]',
  'in-development': 'border-crimson-deep text-crimson-deep',
  prototype: 'border-[#2b2218] text-[#2b2218]',
  planned: 'border-[#2b2218]/50 text-[#2b2218]/70 border-dashed',
};

export const TechnologySection: React.FC = () => (
  <section id={SECTION_IDS.technology} aria-labelledby="technology-heading" className="relative overflow-hidden border-b border-ink-line bg-ink-950 py-section">
    <div className="relative mx-auto max-w-wide px-gutter">
      <EditorialHeading
        id="technology-heading"
        index="08"
        kicker="Technology"
        title="Built to be checked,"
        emphasis="not just believed."
        lead="A forensic pipeline where every step leaves a trace — and an honest ledger of what exists today."
      />

      <div className="mt-16 grid gap-12 lg:grid-cols-12">
        <Reveal className="lg:col-span-7">
          <ol className="relative border-l border-crimson/50 pl-8">
            {PIPELINE.map((step) => (
              <li key={step.index} className="relative pb-8 last:pb-0">
                <span aria-hidden="true" className="absolute -left-[2.35rem] top-1.5 h-3 w-3 rounded-full border-2 border-ink-950 bg-crimson shadow-[0_0_0_3px_rgba(227,38,54,.2)]" />
                <p className="flex items-baseline gap-4">
                  <span className="font-mono text-[0.6875rem] text-crimson">{step.index}</span>
                  <span className="font-serif text-2xl text-fg-bright">{step.title}</span>
                </p>
                <p className="mt-1 max-w-md pl-9 text-sm leading-relaxed text-fg-muted">{step.line}</p>
              </li>
            ))}
          </ol>

          <dl className="mt-14 grid gap-px border border-ink-line bg-ink-line sm:grid-cols-2">
            {STACK.map((item) => (
              <div key={item.layer} className="bg-ink-900 px-5 py-4">
                <dt className="font-mono text-[0.625rem] uppercase tracking-[0.14em] text-fg-subtle">{item.layer}</dt>
                <dd className="mt-1.5 text-sm text-fg">{item.tech}</dd>
                {item.note && <dd className="mt-0.5 font-mono text-[0.625rem] uppercase tracking-[0.1em] text-paper/80">{item.note}</dd>}
              </div>
            ))}
          </dl>
        </Reveal>

        <Reveal delay={120} className="lg:col-span-5">
          <div
            id="build-status"
            className="relative rotate-[0.6deg] bg-[#e6d6b8] p-7 text-[#2b2218] shadow-[inset_0_0_24px_rgba(98,66,30,.35),0_40px_60px_-24px_rgba(0,0,0,.9)] [background-image:var(--fibre),linear-gradient(170deg,#efe2c8,#d9c5a2)]"
          >
            <Pushpin className="-top-4 left-1/2 -translate-x-1/2" />
            <div className="flex items-baseline justify-between border-b border-[#2b2218]/30 pb-3">
              <h3 className="font-serif text-2xl">Build status</h3>
              <span className="font-mono text-[0.625rem] uppercase tracking-[0.14em] text-[#2b2218]/60">Ledger · 2026</span>
            </div>
            <ul className="divide-y divide-[#2b2218]/15">
              {DELIVERABLES.map((item) => (
                <li key={item.id} id={`capability-${item.id}`} className="flex items-start justify-between gap-4 py-4">
                  <div>
                    <p className="font-serif text-[1.05rem] leading-tight">{item.title}</p>
                    <p className="mt-1 text-[0.8125rem] leading-snug text-[#2b2218]/70">{item.description}</p>
                    <p className="mt-1 font-mono text-[0.5625rem] uppercase tracking-[0.1em] text-[#2b2218]/55">{item.statusNote}</p>
                  </div>
                  <span className={`mt-0.5 shrink-0 -rotate-3 border-2 px-2 py-0.5 font-mono text-[0.625rem] font-medium uppercase tracking-[0.12em] ${STAMP[item.status]}`}>
                    {DELIVERABLE_STATUS_LABEL[item.status]}
                  </span>
                </li>
              ))}
            </ul>
            <p className="mt-2 border-t border-[#2b2218]/30 pt-3 text-[0.75rem] leading-snug text-[#2b2218]/70">
              Status reflects the repository today. Demonstrations on this page use synthetic data only.
            </p>
          </div>
        </Reveal>
      </div>
    </div>
  </section>
);
