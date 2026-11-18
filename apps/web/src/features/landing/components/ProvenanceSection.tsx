import React, { useState } from 'react';
import { ChevronRight, CircleDashed, FileSearch, Fingerprint, GitCompareArrows, Link2, UserCheck, type LucideIcon } from 'lucide-react';
import { SECTION_IDS } from '../data/siteContent';
import { Container, ProvenanceBadge, Reveal, ReviewBadge, SectionHeader } from '../../../components/ui/primitives';

interface ChainLink {
  id: string;
  step: string;
  title: string;
  body: React.ReactNode;
  reference: string;
}

const CHAIN: ChainLink[] = [
  {
    id: 'artifact',
    step: 'Original artifact',
    title: 'door_controller_export.csv',
    body: (
      <p className="font-mono text-xs leading-relaxed text-fg-muted">
        EV-02 · read-only original
        <br />
        SHA-256 3b77…0c92
        <br />
        acquired 2026-03-16 10:40 UTC
      </p>
    ),
    reference: 'Stored unchanged',
  },
  {
    id: 'fact',
    step: 'Extracted fact',
    title: 'Badge 4471 granted at Gate B, 22:43:52',
    body: <ProvenanceBadge value="observed" />,
    reference: '← EV-02 · row 3,118',
  },
  {
    id: 'entity',
    step: 'Correlated entity / event',
    title: 'Person A ↔ Badge 4471 ↔ Gate B entry',
    body: (
      <span className="flex flex-wrap gap-1.5">
        <ProvenanceBadge value="supplied" />
        <ProvenanceBadge value="observed" />
      </span>
    ),
    reference: '← HR record · EV-02',
  },
  {
    id: 'lead',
    step: 'Investigative lead',
    title: 'Verify whether Person A entered Gate B between 22:41 and 22:45',
    body: <ReviewBadge value="pending" />,
    reference: '← EVT-4 · R1 · DX-01',
  },
];

const PRINCIPLES: { icon: LucideIcon; title: string; body: string }[] = [
  { icon: FileSearch, title: 'Provenance', body: 'Every value knows its artifact.' },
  { icon: Link2, title: 'Source references', body: 'Leads cite artifact IDs.' },
  { icon: Fingerprint, title: 'Integrity checks', body: 'SHA-256 recorded at ingestion.' },
  { icon: GitCompareArrows, title: 'Observed ≠ inferred', body: 'Labelled and drawn differently.' },
  { icon: CircleDashed, title: 'Uncertainty kept', body: 'Clock offsets stay visible.' },
  { icon: UserCheck, title: 'Investigator review', body: 'Leads are prompts to verify.' },
];

export const ProvenanceSection: React.FC = () => {
  const [focused, setFocused] = useState<number | null>(null);

  return (
    <section id={SECTION_IDS.provenance} aria-labelledby="provenance-heading" className="border-b border-ink-line py-section">
      <Container>
        <SectionHeader
          id="provenance-heading"
          eyebrow="05 — Traceability & trust"
          lead="Every insight"
          emphasis="should have a source."
          aside={
<p>From artifact to lead, every step stays inspectable.</p>
          }
        />

        <Reveal className="mt-14">
          <ol
            className="grid gap-px border border-ink-line bg-ink-line md:grid-cols-2 xl:grid-cols-4"
            onMouseLeave={() => setFocused(null)}
            aria-label="Provenance chain from artifact to lead"
          >
            {CHAIN.map((link, index) => {
              const traced = focused !== null && index <= focused;
              const isFocus = focused === index;
              return (
                <li key={link.id} className="relative bg-ink-950">
                  <div
                    tabIndex={0}
                    onMouseEnter={() => setFocused(index)}
                    onFocus={() => setFocused(index)}
                    onBlur={() => setFocused(null)}
                    aria-describedby={`chain-ref-${link.id}`}
                    className={`flex h-full flex-col gap-5 p-6 outline-none transition-colors duration-base focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-amber sm:p-7 ${
                      isFocus ? 'bg-ink-900' : traced ? 'bg-ink-900/60' : ''
                    }`}
                  >
                    <span
                      aria-hidden="true"
                      className={`absolute inset-x-0 top-0 h-0.5 transition-colors duration-base ${traced ? 'bg-signal-amber' : 'bg-transparent'}`}
                    />
                    <div className="flex items-center justify-between">
                      <span className={`font-mono text-micro uppercase ${traced ? 'text-signal-amber' : 'text-fg-subtle'}`}>
                        {String(index + 1).padStart(2, '0')} · {link.step}
                      </span>
                      {index < CHAIN.length - 1 && (
                        <ChevronRight aria-hidden="true" className="hidden h-4 w-4 text-fg-subtle xl:block" />
                      )}
                    </div>
                    <p className="text-lg leading-snug text-fg">{link.title}</p>
                    <div>{link.body}</div>
                    <p id={`chain-ref-${link.id}`} className="mt-auto border-t border-ink-line pt-4 font-mono text-micro uppercase text-fg-subtle">
                      {link.reference}
                    </p>
                  </div>
                </li>
              );
            })}
          </ol>
          <p className="mt-4 font-mono text-micro uppercase text-fg-subtle" aria-live="polite">
            {focused === null
              ? 'Hover or focus a step to trace it back to its artifact'
              : focused === 0
                ? 'The original artifact is the root of the chain'
                : `Tracing ${CHAIN[focused]?.step.toLowerCase()} back through ${focused} supporting step${focused > 1 ? 's' : ''} to EV-02`}
          </p>
        </Reveal>

        <Reveal className="mt-12">
          <ul className="grid grid-cols-2 gap-px border border-ink-line bg-ink-line sm:grid-cols-3 lg:grid-cols-6">
            {PRINCIPLES.map((principle) => (
              <li key={principle.title} className="group bg-ink-950 p-4 transition-colors duration-base hover:bg-ink-900 sm:p-5">
                <principle.icon aria-hidden="true" className="h-5 w-5 text-fg-subtle transition-colors group-hover:text-signal-amber" />
                <h3 className="mt-4 text-sm font-medium text-fg">{principle.title}</h3>
                <p className="mt-1 text-xs text-fg-subtle">{principle.body}</p>
              </li>
            ))}
          </ul>
          <p className="mt-4 font-mono text-micro uppercase leading-relaxed text-fg-subtle">
            A digest alone does not establish that the original source is authentic · SUTRAM supports investigators; it does not determine identity or guilt
          </p>
        </Reveal>
      </Container>
    </section>
  );
};
