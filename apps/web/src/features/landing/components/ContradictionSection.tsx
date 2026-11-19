import React from 'react';
import { DEMO_CASE } from '../../../fixtures/synthetic/case026';
import { SECTION_IDS } from '../data/siteContent';
import { useInView } from '../hooks/useInView';
import { CctvStill } from '../board/artwork/CctvStill';
import { ACCESS_LOG_ROWS, LogPrintout } from '../board/artwork/Documents';
import { EditorialHeading, EvidencePrint, Pushpin, SyntheticStamp } from '../board/materials';
import { useMediaQuery } from '../board/hooks';

const DX = DEMO_CASE.discrepancies[0]!;

const SourceA: React.FC = () => (
  <EvidencePrint rot={-3} tag="EV-01" caption="Source A · camera clock" imageClassName="aspect-[16/11]">
    <CctvStill annotate />
  </EvidencePrint>
);

const SourceB: React.FC = () => (
  <div className="relative rotate-2">
    <Pushpin tone="steel" className="-top-3 left-1/2 -translate-x-1/2" />
    <LogPrintout title="Access control log" file="door_controller_export.csv" tag="EV-02" rows={ACCESS_LOG_ROWS} className="shadow-[0_30px_40px_-16px_rgba(0,0,0,.85)]" />
    <p className="mt-2 font-mono text-[0.625rem] uppercase tracking-[0.12em] text-fg-subtle">Source B · controller clock</p>
  </div>
);

const EventDisc: React.FC = () => (
  <div className="flex flex-col items-center text-center">
    <span className="relative grid h-[4.5rem] w-[4.5rem] place-items-center rounded-full border border-paper/40 bg-ink-800 font-mono text-[0.5625rem] uppercase tracking-[0.14em] text-paper shadow-[0_20px_40px_-10px_rgba(0,0,0,.9)]">
      Event
    </span>
    <p className="mt-3 font-serif text-lg text-fg-bright">Gate B entry</p>
    <p className="font-mono text-[0.625rem] uppercase tracking-[0.12em] text-fg-subtle">Linked by location &amp; time window</p>
  </div>
);

/** Pin head drawn in the thread layer, where a string is tied off on a frame edge. */
const BoardPin: React.FC<{ x: number; y: number; tone?: 'crimson' | 'steel' }> = ({ x, y, tone = 'crimson' }) => (
  <g>
    <ellipse cx={x + 2} cy={y + 4} rx="6" ry="3" fill="#000" opacity="0.45" />
    <circle cx={x} cy={y} r="6" fill={tone === 'crimson' ? '#d0202f' : '#9aa5b1'} />
    <circle cx={x - 2} cy={y - 2} r="2" fill="#fff" opacity="0.6" />
  </g>
);

/** Desktop composition: intact thread from A; the thread from B snaps under the time conflict. */
const DesktopComposition: React.FC<{ drawn: boolean }> = ({ drawn }) => (
  <div className="relative mx-auto aspect-[1200/600] w-full max-w-[75rem]">
    <svg aria-hidden="true" viewBox="0 0 1200 600" className="absolute inset-0 z-10 h-full w-full overflow-visible" data-drawn={drawn}>
      {/* A → event: holds. Tied off at the print's right edge so the string never crosses the photograph. */}
      <path d="M 364 228 Q 460 350 562 380" className="thread-shadow" />
      <path d="M 364 228 Q 460 350 562 380" pathLength={1} className="thread thread--draw" data-state="hot" />
      <BoardPin x={364} y={228} />
      <text x="420" y="246" fontFamily="JetBrains Mono, monospace" fontSize="12" letterSpacing="0.08em" fill="#F1E9DB" className="svg-label" style={{ stroke: '#0b0a09' }}>
        {DX.recordA.value} · CAMERA CLOCK
      </text>

      {/* B → event: snaps. Two halves of one string, each recoiling from the break. */}
      <g className="snap-end snap-end--b" style={{ transformBox: 'fill-box', transformOrigin: '100% 0%' }}>
        <path d="M 880 158 Q 836 214 778 276" className="thread-shadow" />
        <path d="M 880 158 Q 836 214 778 276" pathLength={1} className="thread thread--draw" style={{ '--draw-delay': '300ms' } as React.CSSProperties} />
        <path d="M 778 276 l -7 2 M 778 276 l -5 6 M 778 276 l -8 -2" stroke="#c81f2e" strokeWidth="1" />
      </g>
      <g className="snap-end snap-end--a" style={{ transformBox: 'fill-box', transformOrigin: '0% 100%' }}>
        <path d="M 762 292 Q 700 340 636 378" className="thread-shadow" />
        <path d="M 762 292 Q 700 340 636 378" pathLength={1} className="thread thread--draw" style={{ '--draw-delay': '300ms' } as React.CSSProperties} />
        <path d="M 762 292 l 7 -2 M 762 292 l 4 -6 M 762 292 l 8 1" stroke="#c81f2e" strokeWidth="1" />
      </g>
      <BoardPin x={880} y={158} tone="steel" />
      <text x="896" y="232" fontFamily="JetBrains Mono, monospace" fontSize="12" letterSpacing="0.08em" fill="#F1E9DB" className="svg-label" style={{ stroke: '#0b0a09' }}>
        {DX.recordB.value} · CONTROLLER CLOCK
      </text>

      {/* discrepancy flag at the break */}
      <g className="snap-flag">
        <circle cx="770" cy="284" r="34" fill="#E32636" opacity="0.14" />
        <circle cx="770" cy="284" r="20" fill="#E32636" />
        <text x="770" y="292" textAnchor="middle" fontFamily="Fraunces, Georgia, serif" fontSize="24" fill="#fff">
          ≠
        </text>
        <text x="808" y="330" fontFamily="JetBrains Mono, monospace" fontSize="13" letterSpacing="0.08em" fill="#ff6b76" className="svg-label" style={{ stroke: '#0b0a09' }}>
          {DX.variance.toUpperCase()}
        </text>
        <text x="808" y="348" fontFamily="JetBrains Mono, monospace" fontSize="10" letterSpacing="0.1em" fill="#A9A197" className="svg-label" style={{ stroke: '#0b0a09' }}>
          REQUIRES INVESTIGATOR REVIEW
        </text>
      </g>
    </svg>

    <div className="absolute left-0 top-[6%] w-[30%]">
      <SourceA />
    </div>
    <div className="absolute right-0 top-[10%] w-[27%]">
      <SourceB />
    </div>
    <div className="absolute left-1/2 top-[58%] z-20 -translate-x-1/2">
      <EventDisc />
    </div>
  </div>
);

const CompactComposition: React.FC<{ drawn: boolean }> = ({ drawn }) => (
  <div className="mx-auto max-w-md space-y-12">
    <SourceA />
    <SourceB />
    <svg aria-hidden="true" viewBox="0 0 200 120" className="mx-auto h-28 w-48" data-drawn={drawn}>
      <path d="M 60 0 L 60 100" pathLength={1} className="thread thread--draw" />
      <g className="snap-end snap-end--b" style={{ transformBox: 'fill-box', transformOrigin: '50% 0%' }}>
        <path d="M 140 0 L 140 46" pathLength={1} className="thread thread--draw" />
      </g>
      <g className="snap-end snap-end--a" style={{ transformBox: 'fill-box', transformOrigin: '50% 100%' }}>
        <path d="M 140 62 L 140 100" pathLength={1} className="thread thread--draw" />
      </g>
      <g className="snap-flag">
        <circle cx="140" cy="54" r="12" fill="#E32636" />
        <text x="140" y="59" textAnchor="middle" fontSize="15" fill="#fff">
          ≠
        </text>
      </g>
    </svg>
    <EventDisc />
  </div>
);

export const ContradictionSection: React.FC = () => {
  const [ref, visible] = useInView<HTMLDivElement>({ rootMargin: '0px 0px -25% 0px' });
  const desktop = useMediaQuery('(min-width: 900px)', true);
  return (
    <section id={SECTION_IDS.contradictions} aria-labelledby="contradiction-heading" className="relative overflow-hidden border-b border-ink-line bg-ink-950 py-section">
      <div aria-hidden="true" className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_50%_40%_at_66%_45%,rgba(159,28,38,.18),transparent_70%)]" />
      <div className="relative mx-auto max-w-wide px-gutter">
        <EditorialHeading
          id="contradiction-heading"
          index="05"
          kicker="Contradiction detection"
          title="Not every connection"
          emphasis="is a confirmation."
          lead="Surface conflicts, inconsistencies and information requiring investigator review."
        />

        <div
          ref={ref}
          className="mt-16"
          role="img"
          aria-label={`${DX.recordA.label} at ${DX.recordA.value} (camera clock) versus ${DX.recordB.label} at ${DX.recordB.value} (controller clock): ${DX.variance}. Both link to the same Gate B entry; clock synchronisation requires verification.`}
        >
          {desktop ? <DesktopComposition drawn={visible} /> : <CompactComposition drawn={visible} />}
        </div>

        <dl className="mt-14 grid gap-px border border-ink-line bg-ink-line sm:grid-cols-3">
          <div className="bg-ink-950 p-6">
            <dt className="font-mono text-[0.625rem] uppercase tracking-[0.14em] text-fg-subtle">Variance</dt>
            <dd className="mt-2 font-serif text-3xl text-crimson">{DX.variance}</dd>
            <dd className="mt-1 text-sm text-fg-muted">{DX.recordA.evidenceId} vs {DX.recordB.evidenceId} · same gate, same window</dd>
          </div>
          <div className="bg-ink-950 p-6">
            <dt className="font-mono text-[0.625rem] uppercase tracking-[0.14em] text-fg-subtle">Possible explanations · not assessed</dt>
            <dd className="mt-2 text-sm leading-relaxed text-fg">Camera clock drift · controller clock drift · two separate entries</dd>
          </div>
          <div className="bg-ink-950 p-6">
            <dt className="font-mono text-[0.625rem] uppercase tracking-[0.14em] text-fg-subtle">Next step</dt>
            <dd className="mt-2 text-sm leading-relaxed text-fg">{DX.action}</dd>
          </div>
        </dl>
        <div className="mt-6 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <p className="font-serif text-lg italic text-fg-muted">A discrepancy is a question for review — not evidence of wrongdoing.</p>
          <SyntheticStamp />
        </div>
      </div>
    </section>
  );
};
