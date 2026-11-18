import React, { useId, useRef, useState } from 'react';
import { CAPABILITIES, SECTION_IDS, type CapabilityId } from '../data/siteContent';
import {
  ContradictionVisual,
  CorrelationVisual,
  ExplainVisual,
  GraphVisual,
  TimelineVisual,
} from './platformVisuals';
import { Container, Frame, Reveal, SectionHeader } from '../../../components/ui/primitives';

const VISUALS: Record<CapabilityId, { component: React.FC; caption: string }> = {
  correlation: { component: CorrelationVisual, caption: 'correlation · shared signals' },
  graph: { component: GraphVisual, caption: 'knowledge graph · 6 entities' },
  timeline: { component: TimelineVisual, caption: 'reconstruction · event sequence' },
  contradiction: { component: ContradictionVisual, caption: 'review queue · contradiction' },
  explainable: { component: ExplainVisual, caption: 'provenance · trace chain' },
};

export const PlatformSection: React.FC = () => {
  const [activeIndex, setActiveIndex] = useState(0);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const baseId = useId();
  const active = CAPABILITIES[activeIndex] ?? CAPABILITIES[0]!;
  const Visual = VISUALS[active.id].component;

  const focusTab = (index: number): void => {
    const next = (index + CAPABILITIES.length) % CAPABILITIES.length;
    setActiveIndex(next);
    tabRefs.current[next]?.focus();
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number): void => {
    const keys: Record<string, () => void> = {
      ArrowDown: () => focusTab(index + 1),
      ArrowRight: () => focusTab(index + 1),
      ArrowUp: () => focusTab(index - 1),
      ArrowLeft: () => focusTab(index - 1),
      Home: () => focusTab(0),
      End: () => focusTab(CAPABILITIES.length - 1),
    };
    const action = keys[event.key];
    if (action) {
      event.preventDefault();
      action();
    }
  };

  return (
    <section id={SECTION_IDS.platform} aria-labelledby="platform-heading" className="border-b border-ink-line py-section">
      <Container>
        <SectionHeader
          id="platform-heading"
          eyebrow="02 — The platform"
          lead="One investigation."
          emphasis="A connected view of the evidence."
          aside={
<p>Five capabilities, one case model — every view points back to the same artifacts.</p>
          }
        />

        <Reveal className="mt-14 grid border border-ink-line lg:grid-cols-12">
          <div
            role="tablist"
            aria-orientation="vertical"
            aria-label="Platform capabilities"
            className="flex flex-col divide-y divide-ink-line border-b border-ink-line lg:col-span-5 lg:border-b-0 lg:border-r"
          >
            {CAPABILITIES.map((capability, index) => {
              const selected = index === activeIndex;
              return (
                <button
                  key={capability.id}
                  ref={(element) => {
                    tabRefs.current[index] = element;
                  }}
                  id={`${baseId}-tab-${capability.id}`}
                  role="tab"
                  type="button"
                  aria-selected={selected}
                  aria-controls={`${baseId}-panel`}
                  tabIndex={selected ? 0 : -1}
                  onClick={() => setActiveIndex(index)}
                  onKeyDown={(event) => onKeyDown(event, index)}
                  className={`group relative px-6 py-5 text-left transition-colors duration-base focus:outline-none focus-visible:bg-white/[0.03] sm:px-8 ${
                    selected ? 'bg-ink-900' : 'hover:bg-white/[0.02]'
                  }`}
                >
                  <span
                    aria-hidden="true"
                    className={`absolute inset-y-0 left-0 w-0.5 bg-signal-amber transition-opacity duration-base ${
                      selected ? 'opacity-100' : 'opacity-0'
                    }`}
                  />
                  <span className="flex items-baseline gap-4">
                    <span className={`font-mono text-xs ${selected ? 'text-signal-amber' : 'text-fg-subtle'}`}>
                      {capability.letter}
                    </span>
                    <span
                      className={`font-display text-xl font-semibold uppercase leading-tight transition-colors sm:text-2xl ${
                        selected ? 'text-fg' : 'text-fg-muted group-hover:text-fg'
                      }`}
                    >
                      {capability.title}
                    </span>
                  </span>
                  {selected && (
                    <span className="mt-2 block pl-7 text-sm leading-relaxed text-fg-muted">{capability.summary}</span>
                  )}
                </button>
              );
            })}
          </div>

          <div
            id={`${baseId}-panel`}
            role="tabpanel"
            aria-labelledby={`${baseId}-tab-${active.id}`}
            tabIndex={0}
            className="relative bg-ink-950 p-4 focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-amber sm:p-8 lg:col-span-7"
          >
            <div aria-hidden="true" className="pattern-grid pointer-events-none absolute inset-0 opacity-40" />
            <Frame
              className="relative"
              title={
                <>
                  <span className="text-signal-amber">{active.letter}</span>
                  <span>{VISUALS[active.id].caption}</span>
                </>
              }
              meta="Synthetic"
              bodyClassName="p-4 sm:p-6"
            >
              <div key={active.id} className="animate-[fadeIn_400ms_cubic-bezier(0.22,1,0.36,1)]">
                <Visual />
              </div>
            </Frame>
          </div>
        </Reveal>
      </Container>
    </section>
  );
};
