import React, { useState } from 'react';
import { FileLock2, Lightbulb, ListTree, ScanText } from 'lucide-react';
import { ProvenanceBadge, ProvenanceLegend } from '../../../components/ui/primitives';
import type { ProvenanceClass } from '../../../fixtures/synthetic/case026';
import { findEvidence } from '../../../fixtures/synthetic/case026';
import { SECTION_IDS } from '../data/siteContent';
import { useInView } from '../hooks/useInView';
import { EditorialHeading, Pushpin, SyntheticStamp } from '../board/materials';
import { ThreadCanvas, type ThreadLink } from '../board/ThreadCanvas';

const SOURCE = findEvidence('EV-02')!;

interface ChainStep {
  id: string;
  stage: string;
  icon: typeof FileLock2;
  title: string;
  detail: string;
  provenance: ProvenanceClass;
  meta: string;
}

const STEPS: ChainStep[] = [
  {
    id: 'artifact',
    stage: 'Original artifact',
    icon: FileLock2,
    title: SOURCE.fileName,
    detail: `Read-only original · sha256 ${SOURCE.sha256}`,
    provenance: 'observed',
    meta: `${SOURCE.id} · acquired ${SOURCE.acquiredAt}`,
  },
  {
    id: 'fact',
    stage: 'Extracted fact',
    icon: ScanText,
    title: 'Badge 4471 granted entry at Gate B, 22:43:52',
    detail: 'Value copied with its row reference; clock source recorded.',
    provenance: 'observed',
    meta: `${SOURCE.id} · row 2 · controller clock`,
  },
  {
    id: 'event',
    stage: 'Correlated event',
    icon: ListTree,
    title: 'EVT-4 · Gate B entry',
    detail: 'Linked with CCTV EV-01 by place and time window — variance flagged.',
    provenance: 'observed',
    meta: 'EVT-4 ↔ EVT-3 · Δ +2 min 45 s',
  },
  {
    id: 'lead',
    stage: 'Investigative lead',
    icon: Lightbulb,
    title: 'Verify whether Person A entered Gate B',
    detail: 'Badge 4471 is issued to Person A (supplied). Whoever carried it is not established.',
    provenance: 'inferred',
    meta: 'Requires investigator review',
  },
];

export const ProvenanceChain: React.FC = () => {
  const [ref, visible] = useInView<HTMLDivElement>({ rootMargin: '0px 0px -20% 0px' });
  const [focused, setFocused] = useState<number | null>(null);

  const links: ThreadLink[] = STEPS.slice(1).map((step, i) => ({
    id: `p-${step.id}`,
    from: `p-${STEPS[i]!.id}`,
    to: `p-${step.id}`,
    kind: step.provenance === 'inferred' ? 'inferred' : 'observed',
    delay: i * 260,
    state: focused === null ? 'idle' : i + 1 <= focused ? 'hot' : 'dim',
  }));

  const status =
    focused === null
      ? 'Hover or focus a step to trace it back to its source.'
      : focused === 0
        ? `${STEPS[0]!.stage}: ${SOURCE.id} is the source itself.`
        : `Tracing ${STEPS[focused]!.stage.toLowerCase()} back through ${focused} supporting step${focused === 1 ? '' : 's'} to ${SOURCE.id}.`;

  return (
    <section id={SECTION_IDS.provenance} aria-labelledby="provenance-heading" className="board-surface relative overflow-hidden border-b border-ink-line py-section">
      <div aria-hidden="true" className="board-vignette" />
      <div className="relative mx-auto max-w-wide px-gutter">
        <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <EditorialHeading
            id="provenance-heading"
            index="06"
            kicker="Evidence provenance"
            title="Every insight"
            emphasis="has a source."
            lead="Preserve provenance and distinguish observations from inferred relationships."
          />
          <div className="flex flex-col items-start gap-3 lg:items-end">
            <ProvenanceLegend />
            <SyntheticStamp />
          </div>
        </div>

        <div ref={ref} className="relative mt-16">
          <ThreadCanvas links={links} drawn={visible} sagAmount={0.06} />
          <ol className="relative grid gap-10 md:grid-cols-2 xl:grid-cols-4 xl:gap-8">
            {STEPS.map((step, i) => {
              const Icon = step.icon;
              const lit = focused !== null && i <= focused;
              return (
                <li key={step.id} className="reveal" data-visible={visible} style={{ '--reveal-delay': `${i * 140}ms` } as React.CSSProperties}>
                  <div className="h-full" style={{ transform: `rotate(${[-0.8, 0.6, -0.4, 0.9][i]}deg)` }}>
                  <div
                    tabIndex={0}
                    onMouseEnter={() => setFocused(i)}
                    onMouseLeave={() => setFocused(null)}
                    onFocus={() => setFocused(i)}
                    onBlur={() => setFocused(null)}
                    className={`relative h-full border p-6 transition-[border-color,background-color,transform] duration-slow ease-out focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson ${
                      lit ? 'border-crimson/60 bg-ink-800 xl:-translate-y-1.5' : 'border-ink-strong bg-ink-900/90'
                    } ${focused !== null && !lit ? 'opacity-50' : ''}`}
                  >
                    <Pushpin pinId={`p-${step.id}`} tone={step.provenance === 'inferred' ? 'brass' : 'crimson'} className="-top-4 left-8" />
                    <div className="flex items-center justify-between">
                      <span className="font-mono text-[0.625rem] uppercase tracking-[0.16em] text-fg-subtle">
                        <span className="text-crimson">0{i + 1}</span> · {step.stage}
                      </span>
                      <Icon aria-hidden="true" strokeWidth={1.4} className={`h-5 w-5 ${lit ? 'text-crimson' : 'text-fg-subtle'}`} />
                    </div>
                    <p className={`mt-5 font-serif text-xl leading-snug text-fg-bright ${step.id === 'artifact' ? 'break-all font-mono text-base' : ''}`}>{step.title}</p>
                    <p className="mt-3 text-sm leading-relaxed text-fg-muted">{step.detail}</p>
                    <div className="mt-5 flex flex-wrap items-center justify-between gap-2 border-t border-ink-line pt-4">
                      <ProvenanceBadge value={step.provenance} withDescription />
                      <span className="font-mono text-[0.625rem] uppercase tracking-[0.08em] text-fg-subtle">{step.meta}</span>
                    </div>
                  </div>
                  </div>
                </li>
              );
            })}
          </ol>
        </div>

        <p aria-live="polite" className="mt-10 border-l border-crimson/60 pl-4 font-mono text-[0.6875rem] uppercase tracking-[0.12em] text-fg-muted">
          {status}
        </p>
        <p className="mt-6 max-w-3xl text-sm leading-relaxed text-fg-subtle">
          A SHA-256 digest recorded at intake shows the working copy is unchanged since acquisition — a digest alone does not establish that the original source is authentic. YUKTI surfaces leads for review; it does not determine identity or guilt.
        </p>
      </div>
    </section>
  );
};
