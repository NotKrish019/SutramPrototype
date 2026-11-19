import React, { useState } from 'react';
import { DEMO_CASE, findEvidence } from '../../../fixtures/synthetic/case026';
import { SECTION_IDS } from '../data/siteContent';
import { useInView } from '../hooks/useInView';
import { EditorialHeading, Pushpin, SyntheticStamp } from '../board/materials';
import { ThreadCanvas, type ThreadLink } from '../board/ThreadCanvas';

type Status = 'verified' | 'uncertain' | 'conflict' | 'pending';

const STATUS: Record<Status, { label: string; chip: string; dot: string }> = {
  verified: { label: 'Reviewed', chip: 'border-fg/40 text-fg', dot: 'bg-fg' },
  uncertain: { label: 'Supplied · minute precision', chip: 'border-dashed border-steel/70 text-steel', dot: 'bg-transparent ring-1 ring-steel' },
  conflict: { label: 'Clock conflict', chip: 'border-crimson text-crimson', dot: 'bg-crimson' },
  pending: { label: 'Pending review', chip: 'border-paper/50 text-paper', dot: 'bg-paper' },
};

function statusOf(eventId: string): Status {
  const event = DEMO_CASE.events.find((e) => e.id === eventId)!;
  if (event.provenance === 'supplied') return 'uncertain';
  const evidence = findEvidence(event.evidenceId);
  if (evidence?.review === 'flagged') return 'conflict';
  if (evidence?.review === 'pending') return 'pending';
  return 'verified';
}

const seconds = (t: string): number => {
  const [h = 0, m = 0, s = 0] = t.split(':').map(Number);
  return h * 3600 + m * 60 + s;
};

/** Axis position (%): 21:58 sits before a scale break; 22:36:14 → 22:44:10 is true to scale. */
const SCALE_START = seconds('22:36:14');
const SCALE_END = seconds('22:44:10');
function axisX(time: string): number {
  const t = seconds(time);
  if (t < SCALE_START) return 6;
  return 28 + ((t - SCALE_START) / (SCALE_END - SCALE_START)) * 66;
}

const TOP_ROW = ['EVT-1', 'EVT-3', 'EVT-5'];

const MINUTE_TICKS = ['22:37', '22:38', '22:39', '22:40', '22:41', '22:42', '22:43', '22:44'];

const DISCREPANCY = DEMO_CASE.discrepancies[0]!;

export const TimelineSection: React.FC = () => {
  const [ref, visible] = useInView<HTMLDivElement>({ rootMargin: '0px 0px -15% 0px' });
  const [hover, setHover] = useState<string | null>(null);

  const links: ThreadLink[] = DEMO_CASE.events.map((event, i) => ({
    id: event.id,
    from: `t-card-${event.id}`,
    to: `t-mark-${event.id}`,
    delay: 600 + i * 160,
    state: hover ? (hover === event.id ? 'hot' : 'dim') : 'idle',
  }));

  const x3 = axisX(DISCREPANCY.recordA.value);
  const x4 = axisX(DISCREPANCY.recordB.value);

  return (
    <section id={SECTION_IDS.timeline} aria-labelledby="timeline-heading" className="board-surface relative overflow-hidden border-b border-ink-line py-section">
      <div aria-hidden="true" className="board-vignette" />
      <div className="relative mx-auto max-w-wide px-gutter">
        <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <EditorialHeading
            id="timeline-heading"
            index="04"
            kicker="Timeline reconstruction"
            title="Reconstruct the sequence."
            emphasis="Understand the connections."
            lead="Bring scattered events into a source-backed chronological view."
          />
          <ul aria-label="Event status legend" className="flex flex-wrap gap-2">
            {(Object.keys(STATUS) as Status[]).map((s) => (
              <li key={s} className={`border px-2 py-1 font-mono text-[0.625rem] uppercase tracking-[0.1em] ${STATUS[s].chip}`}>
                {STATUS[s].label}
              </li>
            ))}
          </ul>
        </div>

        <div ref={ref} className="relative mt-16 md:h-[34rem]">
          <ThreadCanvas links={links} drawn={visible} sagAmount={0.04} pulse={false} />

          {/* Axis: a taut red thread with minute ticks and a scale break */}
          <div aria-hidden="true" className="absolute bottom-0 left-3 top-0 w-px md:bottom-auto md:left-0 md:right-0 md:top-1/2 md:h-px md:w-auto">
            <div className="absolute inset-0 bg-gradient-to-b from-crimson/0 via-crimson to-crimson/0 shadow-[0_0_8px_rgba(227,38,54,.6)] md:bg-gradient-to-r" />
          </div>
          <div aria-hidden="true" className="absolute inset-x-0 top-1/2 hidden md:block">
            <span className="absolute -top-3 font-mono text-sm text-fg-subtle" style={{ left: '16.5%' }}>
              ⁄⁄
            </span>
            <span className="absolute top-3 -translate-x-1/2 font-mono text-[0.5625rem] uppercase tracking-[0.12em] text-fg-subtle" style={{ left: '17%' }}>
              38 min
            </span>
            {MINUTE_TICKS.map((t) => (
              <span key={t} className="absolute top-0 h-2 w-px bg-fg-subtle/60" style={{ left: `${axisX(`${t}:00`)}%` }} />
            ))}
            {/* discrepancy bracket */}
            <div className="absolute top-8 border-x border-b border-crimson/80" style={{ left: `${x3}%`, width: `${x4 - x3}%`, height: '0.75rem' }} />
            <p
              className="absolute top-[3.25rem] -translate-x-1/2 whitespace-nowrap font-mono text-[0.625rem] uppercase tracking-[0.12em] text-crimson"
              style={{ left: `${(x3 + x4) / 2}%` }}
            >
              Δ {DISCREPANCY.variance} · clocks unverified
            </p>
          </div>

          <ol className="relative space-y-10 pl-10 md:static md:space-y-0 md:pl-0">
            {DEMO_CASE.events.map((event, i) => {
              const status = statusOf(event.id);
              const top = TOP_ROW.includes(event.id);
              const x = axisX(event.time);
              const cardLeft = Math.min(Math.max(x - 9, 0), 82);
              const evidence = findEvidence(event.evidenceId);
              return (
                <li
                  key={event.id}
                  className="relative md:static"
                  onMouseEnter={() => setHover(event.id)}
                  onMouseLeave={() => setHover(null)}
                >
                  {/* marker on the axis */}
                  <span
                    data-pin-id={`t-mark-${event.id}`}
                    aria-hidden="true"
                    className={`absolute -left-[2.05rem] top-5 h-3 w-3 rounded-full border-2 border-ink-950 md:left-[var(--mx)] md:top-1/2 md:-translate-x-1/2 md:-translate-y-1/2 ${STATUS[status].dot} ${
                      hover === event.id ? 'scale-150' : ''
                    } transition-transform duration-base`}
                    style={{ '--mx': `${x}%` } as React.CSSProperties}
                  />
                  <article
                    tabIndex={0}
                    onFocus={() => setHover(event.id)}
                    onBlur={() => setHover(null)}
                    aria-label={`${event.time} — ${event.title}. Source ${event.evidenceId}. ${STATUS[status].label}.`}
                    className={`reveal relative bg-[#e6d6b8] p-4 text-[#2b2218] shadow-[inset_0_0_14px_rgba(98,66,30,.3),0_24px_36px_-16px_rgba(0,0,0,.9)] [background-image:var(--fibre),linear-gradient(170deg,#efe2c8,#d9c5a2)] transition-transform duration-slow ease-out focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson md:absolute md:left-[var(--card-left)] md:w-[17%] ${
                      top ? 'md:top-0' : 'md:bottom-0'
                    } ${hover === event.id ? '-translate-y-1.5' : ''} ${status === 'uncertain' ? 'outline-dashed outline-1 outline-offset-[-6px] outline-steel/70' : ''}`}
                    data-visible={visible}
                    style={{ '--reveal-delay': `${i * 140}ms`, '--card-left': `${cardLeft}%` } as React.CSSProperties}
                  >
                    <Pushpin pinId={`t-card-${event.id}`} tone={status === 'conflict' ? 'crimson' : 'steel'} className={top ? 'left-1/2 -bottom-4 -translate-x-1/2 rotate-180' : '-top-4 left-1/2 -translate-x-1/2'} />
                    <p className="flex items-baseline justify-between gap-2 font-mono text-[0.625rem] uppercase tracking-[0.12em]">
                      <span className={status === 'conflict' ? 'text-crimson-deep' : 'text-[#2b2218]/70'}>{event.evidenceId}</span>
                      <span className="text-[#2b2218]/50">{event.id}</span>
                    </p>
                    <p className={`mt-1 font-mono text-xl ${status === 'conflict' ? 'text-crimson-deep' : ''}`}>{event.time}</p>
                    <p className="mt-1 font-serif text-[0.95rem] leading-snug">{event.title}</p>
                    <p className="mt-2 truncate font-mono text-[0.5625rem] uppercase tracking-[0.08em] text-[#2b2218]/60">{evidence?.label}</p>
                    {event.note && <p className="mt-1 text-[0.6875rem] italic leading-snug text-crimson-deep">{event.note}</p>}
                  </article>
                </li>
              );
            })}
          </ol>
        </div>

        <div className="mt-12 flex flex-col gap-4 border-t border-ink-line pt-6 sm:flex-row sm:items-center sm:justify-between">
          <p className="max-w-2xl text-sm leading-relaxed text-fg-muted">
            True to scale from 22:36 to 22:44. Times are shown as recorded by each source’s own clock — order between conflicting clocks stays <span className="text-crimson">uncertain</span> until verified.
          </p>
          <SyntheticStamp>Synthetic case CASE-026</SyntheticStamp>
        </div>
      </div>
    </section>
  );
};
