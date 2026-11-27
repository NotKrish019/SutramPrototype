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
import { useIsSynthetic } from '../data/WorkspaceDataContext';

const MONO = 'JetBrains Mono, ui-monospace, monospace';

export type RangeKey = 'full' | 'focus';
interface Range {
  label: string;
  start: string;
  end: string;
  tickMinutes: number;
}

const MIN = 60_000;
const floorTo = (ms: number, minutes: number) => Math.floor(ms / (minutes * MIN)) * minutes * MIN;
const ceilTo = (ms: number, minutes: number) => Math.ceil(ms / (minutes * MIN)) * minutes * MIN;
const rangeLabelFmt = new Intl.DateTimeFormat('en-GB', { timeZone: 'Asia/Kolkata', hour: '2-digit', minute: '2-digit', hour12: false });

/**
 * Both time ranges are derived from the case's own events (never fixed dates), so live cases
 * render on their real time axis. "Focus" frames the conflicting events, or the latest cluster.
 */
export function computeRanges(events: WorkspaceEvent[], conflictEventIds: Set<string>): Record<RangeKey, Range> {
  const ms = events.map((e) => Date.parse(e.timestamp)).filter((t) => Number.isFinite(t));
  if (ms.length === 0) {
    const now = floorTo(Date.now(), 10);
    const empty = { start: new Date(now).toISOString(), end: new Date(now + 60 * MIN).toISOString(), tickMinutes: 10 };
    return { full: { ...empty, label: 'Full span' }, focus: { ...empty, label: 'Focus' } };
  }
  const make = (lo: number, hi: number): Range => {
    const span = hi - lo;
    const tick = span <= 25 * MIN ? 5 : span <= 90 * MIN ? 10 : span <= 6 * 60 * MIN ? 30 : 120;
    return {
      start: new Date(lo).toISOString(),
      end: new Date(hi).toISOString(),
      tickMinutes: tick,
      label: `${rangeLabelFmt.format(lo)} – ${rangeLabelFmt.format(hi)}`,
    };
  };
  const full = make(floorTo(Math.min(...ms) - 5 * MIN, 10), ceilTo(Math.max(...ms) + 5 * MIN, 10));
  const focusMs = events.filter((e) => conflictEventIds.has(e.event_id)).map((e) => Date.parse(e.timestamp));
  const anchor = focusMs.length > 0 ? focusMs : [Math.max(...ms)];
  const focus = make(floorTo(Math.min(...anchor) - 10 * MIN, 10), ceilTo(Math.max(...anchor) + 5 * MIN, 10));
  return { full, focus };
}

const CHART = { width: 1000, labelWidth: 178, laneHeight: 70, axisHeight: 44, right: 28 };

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
  const r = computeRanges(detail.events, new Set(conflicts.flatMap((c) => [c.a, c.b])))[range];
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
          <rect width="6" height="6" fill="#C8CAD0" fillOpacity="0.08" />
          <line x1="0" y1="0" x2="0" y2="6" stroke="#C8CAD0" strokeOpacity="0.45" strokeWidth="1.5" />
        </pattern>
        <pattern id="gap-hatch" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <line x1="0" y1="0" x2="0" y2="8" stroke="#3A3A41" strokeWidth="1" />
        </pattern>
        <marker id="correction-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="6" markerHeight="6" orient="auto">
          <path d="M0 0 L8 4 L0 8 z" fill="#A1A1AA" />
        </marker>
      </defs>

      {/* Missing intervals: stretches with no records from any source (not absence of activity). */}
      {(() => {
        const sorted = [...events].map((e) => Date.parse(e.timestamp)).sort((a, b) => a - b);
        const gaps: [number, number][] = [];
        for (let i = 1; i < sorted.length; i++) {
          if (sorted[i]! - sorted[i - 1]! >= 10 * 60_000) gaps.push([sorted[i - 1]!, sorted[i]!]);
        }
        return gaps.map(([g0, g1]) => {
          const gx0 = clampX(x(new Date(g0).toISOString()));
          const gx1 = clampX(x(new Date(g1).toISOString()));
          if (gx1 - gx0 < 40) return null;
          return (
            <g key={g0} aria-hidden="true">
              <rect x={gx0 + 14} y={CHART.axisHeight - 14} width={gx1 - gx0 - 28} height="6" fill="url(#gap-hatch)" />
              <text x={(gx0 + gx1) / 2} y={CHART.axisHeight - 18} textAnchor="middle" fontFamily={MONO} fontSize="9.5" fill="#71717A">
                no records · {Math.round((g1 - g0) / 60_000)} min
              </text>
            </g>
          );
        });
      })()}

      {ticks.map((t) => {
        const tx = CHART.labelWidth + ((t - start) / (end - start)) * plotWidth;
        return (
          <g key={t}>
            <line x1={tx} x2={tx} y1={CHART.axisHeight - 6} y2={height - 6} stroke="#26262B" />
            <text x={tx} y={14} textAnchor="middle" fontFamily={MONO} fontSize="11" fill="#71717A">
              {tickFormatter.format(new Date(t))}
            </text>
          </g>
        );
      })}

      {lanes.map((lane, i) => {
        const y = CHART.axisHeight + i * CHART.laneHeight;
        return (
          <g key={lane.evidence_id}>
            {i % 2 === 1 && <rect x={CHART.labelWidth} y={y} width={plotWidth} height={CHART.laneHeight} fill="#FFFFFF" fillOpacity="0.012" />}
            <line x1="0" x2={CHART.width} y1={y} y2={y} stroke="#26262B" />
            <line x1={CHART.labelWidth} x2={CHART.labelWidth + plotWidth} y1={y + CHART.laneHeight / 2} y2={y + CHART.laneHeight / 2} stroke="#26262B" strokeDasharray="1 5" />
            <text x="12" y={y + 26} fontFamily={MONO} fontSize="12" fill="#FFFFFF">{lane.evidence_id}</text>
            <text x="12" y={y + 42} fontFamily={MONO} fontSize="10" fill="#A1A1AA">
              {lane.source.length > 24 ? `${lane.source.slice(0, 23)}…` : lane.source}
            </text>
            <text x="12" y={y + 56} fontFamily={MONO} fontSize="9" fill="#71717A">
              {(lane.source_type ?? 'UNCLASSIFIED').replace(/_/g, ' ')}
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
            <path d={`M${ax} ${ay} C ${ax + 40} ${ay}, ${bx - 40} ${by}, ${bx} ${by}`} fill="none" stroke="#E5243B" strokeWidth="1.5" strokeDasharray="6 4" />
            <text x={Math.max(ax, bx) + 14} y={(ay + by) / 2 + 4} fontFamily={MONO} fontSize="11" fill="#E5243B" className="svg-label">
              Δ {durationLabel(a.timestamp, b.timestamp)}
            </text>
          </g>
        );
      })}

      {events.map((event, index) => {
        const y = laneY(event.source_evidence_id);
        const ex = x(event.timestamp);
        const inRange = ex >= CHART.labelWidth && ex <= CHART.labelWidth + plotWidth;
        const selected = event.event_id === selectedId;
        const conflicted = conflicts.some((c) => c.a === event.event_id || c.b === event.event_id);
        const color = selected ? '#FFFFFF' : conflicted ? '#E5243B' : '#EDEDEF';
        const uncertainty = event.uncertainty;
        const concurrent = event.ordering_status === 'CONCURRENT_UNCERTAIN';
        const assumed = (event.assumptions?.length ?? 0) > 0;
        const offset = event.clock_offset_seconds ?? 0;
        const rawX = offset !== 0 ? x(new Date(Date.parse(event.timestamp) - offset * 1000).toISOString()) : null;
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
            className="ws-fade cursor-pointer focus:outline-none [&:focus-visible_.marker]:stroke-signal-accent"
            style={{ ['--i' as string]: index } as React.CSSProperties}
          >
            {uncertainty && (
              <rect
                x={clampX(x(uncertainty.window_start_utc))}
                y={y - 9}
                width={Math.max(clampX(x(uncertainty.window_end_utc)) - clampX(x(uncertainty.window_start_utc)), 2)}
                height="18"
                fill="url(#uncertainty-hatch)"
                stroke="#C8CAD0"
                strokeOpacity="0.5"
              />
            )}
            {rawX !== null && inRange && rawX >= CHART.labelWidth && rawX <= CHART.labelWidth + plotWidth && (
              <g aria-hidden="true">
                <circle cx={rawX} cy={y} r="4.5" fill="none" stroke="#A1A1AA" strokeDasharray="2 2" />
                <line x1={rawX} y1={y + 12} x2={ex} y2={y + 12} stroke="#A1A1AA" strokeWidth="1" markerEnd="url(#correction-arrow)" />
                <text x={(rawX + ex) / 2} y={y + 24} textAnchor="middle" fontFamily={MONO} fontSize="9" fill="#A1A1AA">
                  clock {offset > 0 ? '+' : ''}{offset} s
                </text>
              </g>
            )}
            {inRange ? (
              <>
                {selected && <circle cx={ex} cy={y} r="15" fill="none" stroke="#FFFFFF" strokeOpacity="0.35" />}
                {concurrent && <circle cx={ex} cy={y} r="11" fill="none" stroke="#C8CAD0" strokeOpacity="0.7" strokeDasharray="2 3" />}
                <rect className="marker" x={ex - 6} y={y - 6} width="12" height="12" transform={`rotate(45 ${ex} ${y})`} fill={selected ? '#FFFFFF' : conflicted ? '#E5243B' : '#0A0A0B'} stroke={color} strokeWidth="1.8" />
                {assumed && <circle cx={ex + 9} cy={y - 9} r="3" fill="#D9A441" />}
                <text x={ex} y={y - 16} textAnchor="middle" fontFamily={MONO} fontSize="10.5" fill={color} className="svg-label">
                  {formatEventTime(event.timestamp, event.timestamp_precision)}
                </text>
              </>
            ) : (
              <text x={ex < CHART.labelWidth ? CHART.labelWidth + 4 : CHART.labelWidth + plotWidth - 4} y={y + 4} textAnchor={ex < CHART.labelWidth ? 'start' : 'end'} fontFamily={MONO} fontSize="10.5" fill="#A1A1AA">
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
  const synthetic = useIsSynthetic(detail.case);
  return (
    <Panel as="aside" title={`${event.event_id} · event`} className="ws-inspector-enter">
      <div className="border-b border-ink-line px-4 py-4">
        <p className="font-mono text-micro uppercase text-fg-subtle">{event.event_type}</p>
        <p className="mt-1 text-lg text-fg">{event.title}</p>
        <div className="mt-2"><ProvenanceBadge value={event.provenance} withDescription /></div>
      </div>
      <section aria-label="Timestamp transformation" className="px-4 py-4">
        <h3 className="mb-3 font-mono text-micro uppercase text-fg-subtle">Timestamp transformation</h3>
        <ol className="relative space-y-3 border-l border-ink-strong pl-4">
          {[
            { k: 'Recorded by source', v: <span className="font-mono text-xs text-fg">{event.source_recorded_time}</span> },
            { k: 'Interpreted in', v: event.source_timezone },
            {
              k: 'Clock correction',
              v:
                event.clock_offset_seconds !== undefined && event.clock_offset_seconds !== 0
                  ? `${event.clock_offset_seconds > 0 ? '+' : ''}${event.clock_offset_seconds} s declared, applied once`
                  : 'No declared offset',
            },
            { k: 'Normalised', v: <span className="font-mono text-xs text-fg">{formatUtc(event.timestamp)}</span> },
            { k: 'Case time', v: <span className="font-mono text-sm text-fg-bright">{formatEventTime(event.timestamp, event.timestamp_precision)} IST</span> },
          ].map((step, i, all) => (
            <li key={step.k} className="relative ws-rise" style={{ ['--i' as string]: i }}>
              <span
                aria-hidden="true"
                className={`absolute -left-[1.3rem] top-1 h-2 w-2 rounded-full ${i === all.length - 1 ? 'bg-signal-live' : 'border border-fg-subtle bg-ink-950'}`}
              />
              <span className="block font-mono text-micro uppercase text-fg-subtle">{step.k}</span>
              <span className="block break-words text-sm text-fg-muted">{step.v}</span>
            </li>
          ))}
        </ol>
      </section>
      <KeyValueList
        items={[
          ['Precision', `${event.timestamp_precision} · ${PRECISION_LABEL[event.timestamp_precision]}`],
          ['Ordering', event.ordering_status === 'CONCURRENT_UNCERTAIN' ? <span key="o" className="text-paper">CONCURRENT_UNCERTAIN</span> : event.ordering_status],
          ...(event.uncertainty
            ? ([
                ['Window', `${formatEventTime(event.uncertainty.window_start_utc, 'EXACT_SECOND')} – ${formatEventTime(event.uncertainty.window_end_utc, 'EXACT_SECOND')} IST`],
                ['Basis', event.uncertainty.basis],
              ] as [string, string][])
            : []),
          ...(event.location_name ? ([['Location', event.location_name]] as [string, string][]) : []),
        ]}
      />
      {(event.assumptions?.length ?? 0) > 0 && (
        <>
          <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-signal-warn">Assumed values · not observed</h3>
          <ul className="divide-y divide-ink-line">
            {event.assumptions!.map((assumption) => (
              <li key={`${assumption.kind}-${assumption.field}`} className="px-4 py-2.5 text-sm">
                <span className="font-mono text-xs text-fg">{assumption.field} = {assumption.assumed_value}</span>
                <span className="mt-0.5 block text-xs text-fg-muted">{assumption.basis}</span>
              </li>
            ))}
          </ul>
        </>
      )}
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
  const ranges = useMemo(() => computeRanges(detail.events, new Set(conflicts.flatMap((c) => [c.a, c.b]))), [detail.events, conflicts]);
  const lanesRef = useRef<HTMLDivElement>(null);

  // On narrow screens the lanes scroll sideways: bring the selected (or latest) event into view.
  useEffect(() => {
    const container = lanesRef.current;
    if (!container || container.scrollWidth <= container.clientWidth) return;
    const target = selected ?? events[events.length - 1];
    if (!target) return;
    const r = ranges[range];
    const startMs = new Date(r.start).getTime();
    const ratio = (new Date(target.timestamp).getTime() - startMs) / (new Date(r.end).getTime() - startMs);
    const plotRatio = (CHART.labelWidth + Math.min(Math.max(ratio, 0), 1) * (CHART.width - CHART.labelWidth - CHART.right)) / CHART.width;
    container.scrollLeft = Math.max(plotRatio * container.scrollWidth - container.clientWidth / 2, 0);
  }, [selected, events, range, ranges]);
  const uncertainCount = detail.events.filter((e) => e.ordering_status === 'CONCURRENT_UNCERTAIN').length;
  const lanesCount = new Set(events.map((e) => e.source_evidence_id)).size;

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
          title="Temporal reconstruction"
          meta={`${events.length} of ${detail.events.length} events · ${lanesCount} sources · IST · true to scale`}
          actions={
            <div className="flex" role="group" aria-label="Time range">
              {(Object.keys(ranges) as RangeKey[]).map((key) => (
                <button
                  key={key}
                  type="button"
                  aria-pressed={range === key}
                  onClick={() => setRange(key)}
                  className={`h-8 border px-2.5 font-mono text-micro uppercase transition-colors first:border-r-0 ${
                    range === key ? 'border-signal-live/70 bg-signal-live/10 text-fg-bright' : 'border-ink-line text-fg-muted hover:text-fg'
                  }`}
                >
                  {ranges[key].label}
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
            <span className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
              <span className="flex items-center gap-1.5"><span aria-hidden="true" className="inline-block h-2 w-2 rotate-45 border border-fg" />Recorded value</span>
              <span className="flex items-center gap-1.5"><svg aria-hidden="true" width="18" height="9"><rect width="18" height="9" fill="#C8CAD0" fillOpacity="0.15" stroke="#C8CAD0" strokeOpacity="0.5" /></svg>Uncertainty window</span>
              <span className="flex items-center gap-1.5"><span aria-hidden="true" className="inline-block h-2.5 w-2.5 rounded-full border border-dashed border-paper" />Cannot be strictly ordered</span>
              <span className="flex items-center gap-1.5"><span aria-hidden="true" className="inline-block h-1.5 w-1.5 rounded-full bg-signal-warn" />Assumed value</span>
              <span className="text-signal-flag">Δ conflicting sources</span>
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
