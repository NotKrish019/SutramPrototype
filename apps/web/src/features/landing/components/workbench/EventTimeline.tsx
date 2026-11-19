import React from 'react';
import { AlertTriangle } from 'lucide-react';
import { DEMO_CASE, type DemoEvent } from '../../../../fixtures/synthetic/case026';
import { ProvenanceBadge } from '../../../../components/ui/primitives';

const FLAGGED_EVIDENCE = new Set(
  DEMO_CASE.discrepancies.flatMap((d) => [d.recordA.evidenceId, d.recordB.evidenceId]),
);

export interface EventTimelineProps {
  selectedEventId?: string | null;
  highlighted?: ReadonlySet<string>;
  onSelectEvent?: (event: DemoEvent) => void;
  compact?: boolean;
}

/** Chronological event list. Each event keeps its source reference and provenance. */
export const EventTimeline: React.FC<EventTimelineProps> = ({
  selectedEventId = null,
  highlighted,
  onSelectEvent,
  compact = false,
}) => {
  const hasHighlight = highlighted !== undefined && highlighted.size > 0;

  return (
    <ol className="relative" aria-label={`Event timeline, ${DEMO_CASE.events.length} events`}>
      <span aria-hidden="true" className="absolute bottom-3 left-[6.53rem] top-3 w-px bg-ink-strong sm:left-[7.53rem]" />
      {DEMO_CASE.events.map((event) => {
        const selected = event.id === selectedEventId;
        const lit = !hasHighlight || highlighted.has(event.id);
        const flagged = FLAGGED_EVIDENCE.has(event.evidenceId);
        const body = (
          <>
            <time className={`w-[4.75rem] shrink-0 pt-0.5 font-mono text-xs sm:w-[5.5rem] ${selected ? 'text-signal-accent' : 'text-fg-muted'}`}>
              {event.time}
            </time>
            <span
              aria-hidden="true"
              className={`relative z-10 mt-1.5 h-2.5 w-2.5 shrink-0 rotate-45 border ${
                flagged
                  ? 'border-signal-flag bg-signal-flag'
                  : selected
                    ? 'border-signal-accent bg-signal-accent'
                    : 'border-fg bg-ink-950'
              }`}
            />
            <span className="min-w-0 flex-1">
              <span className={`block text-sm leading-snug ${selected ? 'text-fg' : 'text-fg/90'}`}>{event.title}</span>
              <span className="mt-1.5 flex flex-wrap items-center gap-2">
                <span className="font-mono text-micro uppercase text-fg-subtle">src {event.evidenceId}</span>
                {!compact && <ProvenanceBadge value={event.provenance} />}
                {flagged && (
                  <span className="inline-flex items-center gap-1 font-mono text-micro uppercase text-signal-flag">
                    <AlertTriangle aria-hidden="true" className="h-3 w-3" />
                    Clock variance
                  </span>
                )}
              </span>
              {event.note && !compact && <span className="mt-1 block text-xs text-fg-subtle">{event.note}</span>}
            </span>
          </>
        );
        const rowClass = `flex w-full items-start gap-3 px-3 py-3 text-left transition-[background-color,opacity] duration-base sm:gap-4 ${
          lit ? 'opacity-100' : 'opacity-35'
        } ${selected ? 'bg-signal-accent/[0.07]' : ''}`;
        return (
          <li key={event.id}>
            {onSelectEvent ? (
              <button
                type="button"
                aria-pressed={selected}
                onClick={() => onSelectEvent(event)}
                className={`${rowClass} hover:bg-white/[0.03] focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-accent`}
              >
                {body}
              </button>
            ) : (
              <div className={rowClass}>{body}</div>
            )}
          </li>
        );
      })}
    </ol>
  );
};
