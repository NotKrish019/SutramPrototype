import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, ArrowRight, MousePointerClick } from 'lucide-react';
import { ProvenanceBadge, ReviewBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { useCaseIndex, useQuerySelection } from '../caseIndex';
import { FilterChip, KeyValueList, Panel, SelectField } from '../components/ui';
import { CASE_TIME_ZONE, PRECISION_LABEL, durationLabel, formatEventTime, formatUtc } from '../format';
import type { CaseDetail, WorkspaceEvent } from '../types';
import { EvidencePreview } from '../components/EvidencePreview';
import { useWorkspaceRepository } from '../data/WorkspaceDataContext';

const MONO = 'JetBrains Mono, ui-monospace, monospace';

const RANGES = {
  full: { label: '21:50 – 22:50', start: '2026-03-14T16:20:00Z', end: '2026-03-14T17:20:00Z', tickMinutes: 10 },
  focus: { label: '22:30 – 22:50', start: '2026-03-14T17:00:00Z', end: '2026-03-14T17:20:00Z', tickMinutes: 5 },
} as const;
type RangeKey = keyof typeof RANGES;

const CHART = { width: 1000, labelWidth: 170, laneHeight: 64, axisHeight: 34, right: 24 };

const tickFormatter = new Intl.DateTimeFormat('en-GB', { timeZone: CASE_TIME_ZONE, hour: '2-digit', minute: '2-digit', hour12: false });

export interface Conflict {
  a: string;
  b: string;
  reviewId: string;
}

/** True-to-scale event lanes per source. Shared by the workspace and the public site. */
export const Swimlanes: React.FC<{
  detail: CaseDetail;
  events: WorkspaceEvent[];
  range: RangeKey;
  selectedId: string | null;
  onSelect: (id: string) => void;
  conflicts: Conflict[];
  className?: string;
}> = ({ detail, events, range, selectedId, onSelect, conflicts, className = 'min-w-[44rem]' }) => {
  const r = RANGES[range];
  const start = new Date(r.start).getTime();
  const end = new Date(r.end).getTime();
  const plotWidth = CHART.width - CHART.labelWidth - CHART.right;
  const x = (iso: string): number => CHART.labelWidth + ((new Date(iso).getTime() - start) / (end - start)) * plotWidth;
  const clampX = (value: number): number => Math.min(Math.max(value, CHART.labelWidth), CHART.labelWidth + plotWidth);
  const lanes = detail.evidence.filter((e) => events.some((ev) => ev.source_evidence_id === e.evidence_id));
  const laneY = (evidenceId: string): number => CHART.axisHeight + lanes.findIndex((l) => l.evidence_id === evidenceId) * CHART.laneHeight + CHART.laneHeight / 2;
  const height = CHART.axisHeight + lanes.length * CHART.laneHeight + 10;

  const ticks: number[] = [];
  for (let t = start; t <= end; t += r.tickMinutes * 60_000) ticks.push(t);

  const byId = new Map(events.map((e) => [e.event_id, e]));

  return (
    <svg viewBox={`0 0 ${CHART.width} ${height}`} className={`block h-auto w-full ${className}`} role="group" aria-label={`Event swimlanes, ${r.label} IST, true to scale`}>
      <defs>
        <pattern id="uncertainty-hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="6" height="6" fill="#D8C5A7" fillOpacity="0.08" />
          <line x1="0" y1="0" x2="0" y2="6" stroke="#D8C5A7" strokeOpacity="0.45" strokeWidth="1.5" />
        </pattern>
      </defs>

      {ticks.map((t) => {
        const tx = CHART.labelWidth + ((t - start) / (end - start)) * plotWidth;
        return (
          <g key={t}>
            <line x1={tx} x2={tx} y1={CHART.axisHeight - 6} y2={height - 6} stroke="#26221D" />
            <text x={tx} y={18} textAnchor="middle" fontFamily={MONO} fontSize="11" fill="#857D72">
              {tickFormatter.format(new Date(t))}
            </text>
          </g>
        );
      })}

      {lanes.map((lane, i) => {
        const y = CHART.axisHeight + i * CHART.laneHeight;
        return (
          <g key={lane.evidence_id}>
            <line x1="0" x2={CHART.width} y1={y} y2={y} stroke="#26221D" />
            <text x="12" y={y + 24} fontFamily={MONO} fontSize="12" fill="#F1E9DB">{lane.evidence_id}</text>
            <text x="12" y={y + 40} fontFamily={MONO} fontSize="10" fill="#857D72">
              {lane.source.length > 24 ? `${lane.source.slice(0, 23)}…` : lane.source}
            </text>
          </g>
        );
      })}

      {conflicts.map((conflict) => {
        const a = byId.get(conflict.a);
        const b = byId.get(conflict.b);
        if (!a || !b) return null;
        const ax = clampX(x(a.timestamp));
        const bx = clampX(x(b.timestamp));
        const ay = laneY(a.source_evidence_id);
        const by = laneY(b.source_evidence_id);
        return (
          <g key={conflict.reviewId}>
            <path d={`M${ax} ${ay} C ${ax + 40} ${ay}, ${bx - 40} ${by}, ${bx} ${by}`} fill="none" stroke="#E32636" strokeWidth="1.5" strokeDasharray="6 4" />
            <text x={Math.max(ax, bx) + 14} y={(ay + by) / 2 + 4} fontFamily={MONO} fontSize="11" fill="#E32636" className="svg-label">
              Δ {durationLabel(a.timestamp, b.timestamp)}
            </text>
          </g>
        );
      })}

      {events.map((event) => {
        const y = laneY(event.source_evidence_id);
        const ex = x(event.timestamp);
        const inRange = ex >= CHART.labelWidth && ex <= CHART.labelWidth + plotWidth;
        const selected = event.event_id === selectedId;
        const conflicted = conflicts.some((c) => c.a === event.event_id || c.b === event.event_id);
        const color = selected ? '#D8C5A7' : conflicted ? '#E32636' : '#F1E9DB';
        const uncertainty = event.uncertainty;
        return (
          <g
            key={event.event_id}
            role="button"
            tabIndex={0}
            aria-pressed={selected}
            aria-label={`${event.title}, ${formatEventTime(event.timestamp, event.timestamp_precision)} IST${inRange ? '' : ', outside the visible range'}`}
            onClick={() => onSelect(event.event_id)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onSelect(event.event_id);
              }
            }}
            className="cursor-pointer focus:outline-none [&:focus-visible_.marker]:stroke-signal-accent"
          >
            {uncertainty && (
              <rect
                x={clampX(x(uncertainty.window_start_utc))}
                y={y - 9}
                width={Math.max(clampX(x(uncertainty.window_end_utc)) - clampX(x(uncertainty.window_start_utc)), 2)}
                height="18"
                fill="url(#uncertainty-hatch)"
                stroke="#D8C5A7"
                strokeOpacity="0.5"
              />
            )}
            {inRange ? (
              <>
                <rect className="marker" x={ex - 6} y={y - 6} width="12" height="12" transform={`rotate(45 ${ex} ${y})`} fill={selected ? '#D8C5A7' : '#080807'} stroke={color} strokeWidth="1.8" />
                <text x={ex} y={y - 14} textAnchor="middle" fontFamily={MONO} fontSize="10.5" fill={color} className="svg-label">
                  {formatEventTime(event.timestamp, event.timestamp_precision)}
                </text>
              </>
            ) : (
              <text x={ex < CHART.labelWidth ? CHART.labelWidth + 4 : CHART.labelWidth + plotWidth - 4} y={y + 4} textAnchor={ex < CHART.labelWidth ? 'start' : 'end'} fontFamily={MONO} fontSize="10.5" fill="#A9A197">
                {ex < CHART.labelWidth ? '◂ ' : ''}
                {formatEventTime(event.timestamp, event.timestamp_precision)} outside range
                {ex > CHART.labelWidth ? ' ▸' : ''}
              </text>
            )}
            <rect x={inRange ? ex - 22 : CHART.labelWidth} y={y - 24} width={inRange ? 44 : 150} height="44" fill="transparent" />
          </g>
        );
      })}
    </svg>
  );
};

const EventDetailPanel: React.FC<{ event: WorkspaceEvent; detail: CaseDetail }> = ({ event, detail }) => {
  const index = useCaseIndex(detail);
  const caseId = detail.case.case_id;
  const reviews = detail.review_items.filter((item) => item.related_event_ids.includes(event.event_id));
  const evidence = index.evidence.get(event.source_evidence_id);
  const synthetic = useWorkspaceRepository().source === 'synthetic';
  return (
    <Panel as="aside" title={`${event.event_id} · event`} className="ws-inspector-enter">
      <div className="border-b border-ink-line px-4 py-4">
        <p className="font-mono text-micro uppercase text-fg-subtle">{event.event_type}</p>
        <p className="mt-1 text-lg text-fg">{event.title}</p>
        <div className="mt-2"><ProvenanceBadge value={event.provenance} withDescription /></div>
      </div>
      <KeyValueList
        items={[
          ['Case time', `${formatEventTime(event.timestamp, event.timestamp_precision)} IST`],
          ['Normalised', formatUtc(event.timestamp)],
          ['Recorded as', <span key="r" className="font-mono text-xs">{event.source_recorded_time}</span>],
          ['Source zone', event.source_timezone],
          ['Precision', `${event.timestamp_precision} · ${PRECISION_LABEL[event.timestamp_precision]}`],
          ['Ordering', event.ordering_status === 'CONCURRENT_UNCERTAIN' ? <span key="o" className="text-signal-accent">CONCURRENT_UNCERTAIN</span> : event.ordering_status],
          ...(event.uncertainty
            ? ([
                ['Window', `${formatEventTime(event.uncertainty.window_start_utc, 'EXACT_SECOND')} – ${formatEventTime(event.uncertainty.window_end_utc, 'EXACT_SECOND')} IST`],
                ['Basis', event.uncertainty.basis],
              ] as [string, string][])
            : []),
          ...(event.location_name ? ([['Location', event.location_name]] as [string, string][]) : []),
        ]}
      />
      <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle">Supporting artifact</h3>
      <Link to={caseHref(caseId, 'evidence', { evidence: event.source_evidence_id })} className="flex items-center justify-between gap-3 px-4 py-3 hover:bg-white/[0.02]">
        {evidence && <EvidencePreview evidence={evidence} synthetic={synthetic} className="w-24 shrink-0 border border-ink-line" />}
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm text-fg">{evidence?.original_filename}</span>
          <span className="font-mono text-micro text-fg-subtle">{event.source_evidence_id} · {evidence?.source}</span>
        </span>
        <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />
      </Link>
      <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle">Entities</h3>
      <ul className="divide-y divide-ink-line">
        {event.involved_entity_ids.map((id) => (
          <li key={id}>
            <Link to={caseHref(caseId, 'graph', { node: id })} className="flex items-center justify-between px-4 py-2.5 text-sm text-fg hover:bg-white/[0.02]">
              {index.nameOf(id)}
              <span className="font-mono text-micro text-fg-subtle">{index.entities.get(id)?.entity_type ?? 'UNCLASSIFIED'}</span>
            </Link>
          </li>
        ))}
      </ul>
      {reviews.length > 0 && (
        <>
          <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle">Review</h3>
          <ul className="divide-y divide-ink-line">
            {reviews.map((item) => (
              <li key={item.id}>
                <Link to={caseHref(caseId, 'contradictions', { item: item.id })} className="flex items-center justify-between gap-2 px-4 py-2.5 hover:bg-white/[0.02]">
                  <span className="truncate text-sm text-fg">{item.title}</span>
                  <ReviewBadge value={item.status} />
                </Link>
              </li>
            ))}
          </ul>
        </>
      )}
    </Panel>
  );
};

export const EventReconstruction: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const [selectedId, setSelectedId] = useQuerySelection('event');
  const [range, setRange] = useState<RangeKey>('full');
  const [hiddenSources, setHiddenSources] = useState<Set<string>>(() => new Set());
  const [eventType, setEventType] = useState('all');

  const conflicts = useMemo<Conflict[]>(
    () =>
      detail.review_items
        .filter((item) => item.category === 'conflicting' && item.related_event_ids.length === 2)
        .map((item) => ({ a: item.related_event_ids[0]!, b: item.related_event_ids[1]!, reviewId: item.id })),
    [detail.review_items],
  );
  const types = useMemo(() => [...new Set(detail.events.map((e) => e.event_type))].sort(), [detail.events]);
  const events = useMemo(
    () =>
      [...detail.events]
        .filter((e) => !hiddenSources.has(e.source_evidence_id) && (eventType === 'all' || e.event_type === eventType))
        .sort((a, b) => a.timestamp.localeCompare(b.timestamp)),
    [detail.events, hiddenSources, eventType],
  );
  const selected = detail.events.find((e) => e.event_id === selectedId) ?? null;
  const lanesRef = useRef<HTMLDivElement>(null);

  // On narrow screens the lanes scroll sideways: bring the selected (or latest) event into view.
  useEffect(() => {
    const container = lanesRef.current;
    if (!container || container.scrollWidth <= container.clientWidth) return;
    const target = selected ?? events[events.length - 1];
    if (!target) return;
    const r = RANGES[range];
    const startMs = new Date(r.start).getTime();
    const ratio = (new Date(target.timestamp).getTime() - startMs) / (new Date(r.end).getTime() - startMs);
    const plotRatio = (CHART.labelWidth + Math.min(Math.max(ratio, 0), 1) * (CHART.width - CHART.labelWidth - CHART.right)) / CHART.width;
    container.scrollLeft = Math.max(plotRatio * container.scrollWidth - container.clientWidth / 2, 0);
  }, [selected, events, range]);
  const uncertainCount = detail.events.filter((e) => e.ordering_status === 'CONCURRENT_UNCERTAIN').length;

  const toggleSource = (id: string): void =>
    setHiddenSources((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  return (
    <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
      <div className="min-w-0 space-y-4 xl:col-span-8">
        <Panel
          title="Event reconstruction"
          meta={`${events.length} of ${detail.events.length} events · IST · true to scale`}
          actions={
            <div className="flex" role="group" aria-label="Time range">
              {(Object.keys(RANGES) as RangeKey[]).map((key) => (
                <button
                  key={key}
                  type="button"
                  aria-pressed={range === key}
                  onClick={() => setRange(key)}
                  className={`h-8 border px-2.5 font-mono text-micro uppercase transition-colors first:border-r-0 ${
                    range === key ? 'border-signal-accent/60 bg-signal-accent/10 text-fg' : 'border-ink-line text-fg-muted hover:text-fg'
                  }`}
                >
                  {RANGES[key].label}
                </button>
              ))}
            </div>
          }
        >
          <div className="flex flex-col gap-3 border-b border-ink-line p-3 lg:flex-row lg:items-center">
            <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filter by source">
              {detail.evidence.map((e) => (
                <FilterChip key={e.evidence_id} active={!hiddenSources.has(e.evidence_id)} onClick={() => toggleSource(e.evidence_id)}>
                  {e.evidence_id}
                </FilterChip>
              ))}
            </div>
            <SelectField
              label="Type"
              value={eventType}
              onChange={setEventType}
              options={[{ value: 'all', label: 'All event types' }, ...types.map((t) => ({ value: t, label: t }))]}
              className="lg:ml-auto lg:w-60"
            />
          </div>
          <div ref={lanesRef} className="overflow-x-auto bg-ink-950 px-2 py-3">
            <Swimlanes detail={detail} events={events} range={range} selectedId={selectedId} onSelect={setSelectedId} conflicts={conflicts} />
          </div>
          <div className="flex flex-col gap-2 border-t border-ink-line px-4 py-3 text-xs text-fg-muted sm:flex-row sm:items-center sm:justify-between">
            <span className="flex items-center gap-2">
              <svg aria-hidden="true" width="22" height="10"><rect width="22" height="10" fill="#D8C5A7" fillOpacity="0.15" stroke="#D8C5A7" strokeOpacity="0.5" /></svg>
              Uncertainty window · ◆ recorded value · <span className="text-signal-flag">Δ conflicting sources</span>
            </span>
            {uncertainCount > 0 && (
              <span className="flex items-center gap-1.5 text-signal-accent">
                <AlertTriangle aria-hidden="true" className="h-3.5 w-3.5" />
                {uncertainCount} events cannot be strictly ordered
              </span>
            )}
          </div>
        </Panel>

        <Panel title="Chronology" meta="sorted by normalised time">
          <div className="overflow-x-auto">
            <table className="w-full text-left">
              <thead>
                <tr className="border-b border-ink-line font-mono text-micro uppercase text-fg-subtle">
                  <th scope="col" className="px-4 py-2.5 font-normal">Time (IST)</th>
                  <th scope="col" className="px-4 py-2.5 font-normal">Event</th>
                  <th scope="col" className="hidden px-4 py-2.5 font-normal sm:table-cell">Source</th>
                  <th scope="col" className="hidden px-4 py-2.5 font-normal md:table-cell">Precision</th>
                  <th scope="col" className="hidden px-4 py-2.5 font-normal md:table-cell">Ordering</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-ink-line">
                {events.map((event) => {
                  const isSelected = event.event_id === selectedId;
                  return (
                    <tr key={event.event_id} className={isSelected ? 'bg-signal-accent/[0.07]' : 'hover:bg-white/[0.02]'}>
                      <td className="px-4 py-2.5 font-mono text-xs text-fg">{formatEventTime(event.timestamp, event.timestamp_precision)}</td>
                      <td className="px-4 py-2.5">
                        <button type="button" aria-pressed={isSelected} onClick={() => setSelectedId(event.event_id)} className={`text-left text-sm ${isSelected ? 'text-signal-accent' : 'text-fg hover:text-signal-accent'}`}>
                          {event.title}
                        </button>
                        <span className="block font-mono text-micro text-fg-subtle">{event.event_type}<span className="sm:hidden"> · {event.source_evidence_id}</span></span>
                      </td>
                      <td className="hidden px-4 py-2.5 font-mono text-xs text-fg-muted sm:table-cell">{event.source_evidence_id}</td>
                      <td className="hidden px-4 py-2.5 font-mono text-micro uppercase text-fg-muted md:table-cell">
                        {event.timestamp_precision}
                        {event.uncertainty && event.timestamp_precision === 'EXACT_SECOND' && <span className="block text-signal-accent">clock uncertain</span>}
                      </td>
                      <td className={`hidden px-4 py-2.5 font-mono text-micro uppercase md:table-cell ${event.ordering_status === 'CONCURRENT_UNCERTAIN' ? 'text-signal-accent' : 'text-fg-subtle'}`}>
                        {event.ordering_status === 'CONCURRENT_UNCERTAIN' ? 'Uncertain' : 'Ordered'}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </Panel>
      </div>

      <div className="xl:sticky xl:top-4 xl:col-span-4 xl:self-start">
        {selected ? (
          <EventDetailPanel event={selected} detail={detail} />
        ) : (
          <Panel title="Event detail">
            <div className="px-4 py-8 text-center">
              <MousePointerClick aria-hidden="true" strokeWidth={1.2} className="mx-auto h-8 w-8 text-fg-subtle" />
              <p className="mt-3 text-sm text-fg-muted">Select an event.</p>
            </div>
          </Panel>
        )}
      </div>
    </div>
  );
};
