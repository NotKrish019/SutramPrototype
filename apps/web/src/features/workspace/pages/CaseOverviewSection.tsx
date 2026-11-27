import { CommandCentre } from '../../theatre/CommandCentre';
import React, { useMemo } from 'react';
import { AlertTriangle, Archive, ArrowRight, ListOrdered, Share2, type LucideIcon } from 'lucide-react';
import { ReviewBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref, navigate } from '../../../router/router';
import { EvidencePreview } from '../components/EvidencePreview';
import { Panel, ReviewCategoryBadge } from '../components/ui';
import { useIsSynthetic } from '../data/WorkspaceDataContext';
import { formatEventTime } from '../format';
import { CATEGORY_LABEL, buildGraph } from '../graph/graphModel';
import type { CaseDetail, ReviewItem } from '../types';
import { NODE_TONE } from './KnowledgeGraphWorkspace';

const MONO = 'JetBrains Mono, ui-monospace, monospace';

const SectionLink: React.FC<{ to: string; children: React.ReactNode }> = ({ to, children }) => (
  <Link to={to} className="inline-flex items-center gap-1 font-mono text-micro uppercase text-fg-muted hover:text-signal-accent">
    {children}
    <ArrowRight aria-hidden="true" className="h-3 w-3" />
  </Link>
);

/** In-SVG link that uses client-side navigation but stays a real, focusable link. */
const SvgLink: React.FC<{ href: string; label: string; children: React.ReactNode }> = ({ href, label, children }) => (
  <a
    href={href}
    aria-label={label}
    className="cursor-pointer focus:outline-none [&:focus-visible_.focus-ring]:opacity-100"
    onClick={(event) => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return;
      event.preventDefault();
      navigate(href);
    }}
  >
    <title>{label}</title>
    {children}
  </a>
);

/* ------------------------------------------------------- Investigation summary */

const SummaryHero: React.FC<{ detail: CaseDetail; openReview: number; lead: ReviewItem | undefined }> = ({ detail, openReview, lead }) => {
  const caseId = detail.case.case_id;
  const conflicts = detail.review_items.filter((r) => r.category === 'conflicting' && r.status !== 'verified').length;
  const metrics: { label: string; value: number; href: string; icon: LucideIcon; alert?: boolean }[] = [
    { label: 'Evidence files', value: detail.evidence.length, href: caseHref(caseId, 'evidence'), icon: Archive },
    { label: 'Events', value: detail.events.length, href: caseHref(caseId, 'timeline'), icon: ListOrdered },
    { label: 'Entities', value: detail.entities.length, href: caseHref(caseId, 'graph'), icon: Share2 },
    { label: 'Needs review', value: openReview, href: caseHref(caseId, 'contradictions'), icon: AlertTriangle, alert: openReview > 0 },
  ];
  const metricRow = (
    <dl className="grid grid-cols-2 sm:grid-cols-4">
      {metrics.map((metric) => (
        <div key={metric.label}>
          <Link to={metric.href} className="group flex flex-col gap-1.5 px-5 py-4 transition-colors hover:bg-white/[0.03]">
            <dt className="flex items-center gap-1.5 text-xs text-fg-subtle">
              <metric.icon aria-hidden="true" strokeWidth={1.6} className={`h-3.5 w-3.5 ${metric.alert ? 'text-signal-flag' : ''}`} />
              {metric.label}
            </dt>
            <dd className={`text-[1.9rem] font-semibold leading-none tracking-[-0.03em] tabular-nums ${metric.alert ? 'text-signal-flag' : 'text-fg-bright'}`}>
              {String(metric.value).padStart(2, '0')}
            </dd>
          </Link>
        </div>
      ))}
    </dl>
  );
  return (
    <section aria-labelledby="case-summary-heading" className="grid gap-5 xl:col-span-12 xl:grid-cols-12">
      <div className="min-w-0 xl:col-span-8 ws-rise">
        <div className="mb-3 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 px-1">
          <h2 id="case-summary-heading" className="text-sm font-medium text-fg-subtle">
            Investigation summary
          </h2>
          <p className="text-sm text-fg-muted">
            {detail.evidence.length} exhibits → {detail.events.length} events
            {conflicts > 0 ? (
              <span className="text-signal-flag"> · {conflicts} {conflicts === 1 ? 'conflict needs' : 'conflicts need'} an investigator</span>
            ) : (
              <span> · no unresolved conflicts</span>
            )}
          </p>
        </div>
        <CommandCentre detail={detail} footer={metricRow} />
      </div>
      <div className="flex min-w-0 flex-col xl:col-span-4 ws-rise" style={{ ['--i' as string]: 1 }}>
        <p className="mb-3 flex items-center gap-2 px-1 text-sm font-medium text-signal-flag">
          <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-signal-flag" /> Next action
        </p>
        <div className="next-card flex flex-col overflow-hidden rounded-[1.25rem] border border-signal-flag/30">
          {lead ? (
            <>
              <div className="flex-1">
                <ContradictionCompare detail={detail} item={lead} />
              </div>
              <div className="flex flex-col gap-4 px-5 pb-5">
                <p className="text-sm leading-relaxed text-fg-muted">{lead.suggested_action}</p>
                <Link
                  to={caseHref(caseId, 'contradictions', { item: lead.id })}
                  className="inline-flex items-center justify-center gap-2 rounded-xl bg-signal-flag px-4 py-3 text-sm font-semibold text-white shadow-[0_12px_30px_-12px_rgba(229,36,59,0.9)] transition-colors hover:bg-crimson-deep focus:outline-none focus-visible:ring-2 focus-visible:ring-fg-bright"
                >
                  Review this contradiction <ArrowRight aria-hidden="true" className="h-4 w-4" />
                </Link>
              </div>
            </>
          ) : (
            <p className="px-5 py-6 text-sm text-fg-muted">Nothing is waiting for review. Continue with the judicial report.</p>
          )}
        </div>
      </div>
    </section>
  );
};

/* ------------------------------------------------------------ Graph preview */

const GraphPreview: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const caseId = detail.case.case_id;
  const graph = useMemo(() => buildGraph(detail), [detail]);
  // The overview shows entities and events; artifacts live in the gallery below.
  const nodes = graph.nodes.filter((n) => n.category !== 'evidence');
  const ids = new Set(nodes.map((n) => n.id));
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const edges = graph.edges.filter((e) => ids.has(e.source) && ids.has(e.target));
  const categories = [...new Set(nodes.map((n) => n.category))];

  return (
    <Panel title="Relationships" meta={`${nodes.length} nodes · ${edges.length} links`} actions={<SectionLink to={caseHref(caseId, 'graph')}>Open graph</SectionLink>} className="xl:col-span-5">
      <div className="relative bg-ink-950">
        <div aria-hidden="true" className="pattern-grid pointer-events-none absolute inset-0 opacity-40" />
        <svg viewBox="40 50 920 600" className="relative block aspect-[920/600] w-full" role="group" aria-label={`Relationship overview for ${caseId}. Select a node to open it in the knowledge graph.`}>
          {edges.map((edge, i) => {
            const a = byId.get(edge.source)!;
            const b = byId.get(edge.target)!;
            const contradiction = edge.kind === 'CONTRADICTS';
            const dash = edge.provenance === 'inferred' ? '7 6' : edge.provenance === 'supplied' ? '2 5' : undefined;
            return (
              <line
                key={edge.id}
                x1={a.x}
                y1={a.y}
                x2={b.x}
                y2={b.y}
                stroke={contradiction ? '#E5243B' : edge.provenance === 'structural' ? '#71717A' : '#C8CAD0'}
                strokeOpacity={contradiction ? 1 : edge.provenance === 'structural' ? 0.4 : 0.75}
                strokeWidth={contradiction ? 1.8 : 1.2}
                strokeDasharray={contradiction ? '10 4 2 4' : dash}
                pathLength={dash || contradiction ? undefined : 1}
                className={dash || contradiction ? undefined : 'ws-draw'}
                style={{ '--ws-draw-delay': `${i * 40}ms` } as React.CSSProperties}
              />
            );
          })}
          {nodes.map((node) => (
            <SvgLink key={node.id} href={caseHref(caseId, 'graph', { node: node.id })} label={`${node.label}, ${CATEGORY_LABEL[node.category]}`}>
              <circle className="focus-ring" cx={node.x} cy={node.y} r="24" fill="none" stroke="#C8CAD0" strokeDasharray="3 3" opacity="0" />
              <circle cx={node.x} cy={node.y} r={node.category === 'event' ? 7 : 11} fill="#111113" stroke={NODE_TONE[node.category]} strokeWidth="1.8" />
              {node.category !== 'event' && (
                <text x={node.x} y={node.y + 27} textAnchor="middle" className="svg-label" fontFamily={MONO} fontSize="13" fill="#EDEDEF">
                  {node.label.length > 22 ? `${node.label.slice(0, 21)}…` : node.label}
                </text>
              )}
            </SvgLink>
          ))}
        </svg>
        <ul aria-label="Node types" className="pointer-events-none absolute bottom-2 left-2 flex flex-wrap gap-x-3 gap-y-1 bg-ink-950/85 px-2 py-1 font-mono text-[0.625rem] uppercase tracking-[0.08em] text-fg-muted">
          {categories.map((category) => (
            <li key={category} className="flex items-center gap-1.5">
              <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full" style={{ background: NODE_TONE[category] }} />
              {CATEGORY_LABEL[category]}
            </li>
          ))}
          <li className="flex items-center gap-1.5 text-crimson">
            <span aria-hidden="true" className="h-px w-3 bg-crimson" /> Conflict
          </li>
        </ul>
      </div>
    </Panel>
  );
};

/* ---------------------------------------------------- Contradiction compare */

const ContradictionCompare: React.FC<{ detail: CaseDetail; item: ReviewItem }> = ({ detail, item }) => {
  const [a, b] = item.records;
  return (
    <Link
      to={caseHref(detail.case.case_id, 'contradictions', { item: item.id })}
      className="block border-b border-ink-line p-4 transition-colors hover:bg-crimson/[0.04]"
      aria-label={`${item.title}: ${item.records.map((r) => `${r.evidence_id} ${r.value}`).join(' versus ')}`}
    >
      <span className="flex items-center justify-between gap-2">
        <span className="flex items-center gap-2">
          <ReviewCategoryBadge category={item.category} />
          <span className="font-mono text-micro text-fg-subtle">{item.id}</span>
        </span>
        <ReviewBadge value={item.status} />
      </span>
      <span className="mt-2 block text-sm text-fg">{item.title}</span>
      {a && b && (
        <span className="mt-3 grid grid-cols-[1fr_auto_1fr] items-center gap-2">
          {[a, b].map((record, i) => (
            <span key={record.evidence_id} className={`border border-ink-line bg-ink-950 px-2.5 py-2 ${i === 1 ? 'order-3 text-right' : ''}`}>
              <span className="block font-mono text-micro uppercase text-fg-subtle">{record.evidence_id}</span>
              <span className="block font-mono text-sm text-fg-bright">{record.value.replace(' IST', '')}</span>
            </span>
          ))}
          <span className="order-2 flex flex-col items-center">
            <span aria-hidden="true" className="grid h-7 w-7 place-items-center rounded-full bg-crimson font-serif text-base text-white">≠</span>
          </span>
        </span>
      )}
      {item.discrepancy && <span className="mt-2 block font-mono text-micro uppercase text-crimson">{item.discrepancy}</span>}
    </Link>
  );
};

/* ------------------------------------------------------------ Timeline strip */

const TimelineStrip: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const caseId = detail.case.case_id;
  const events = [...detail.events].sort((x, y) => x.timestamp.localeCompare(y.timestamp));
  const conflicted = new Set(detail.review_items.filter((r) => r.category === 'conflicting').flatMap((r) => r.related_event_ids));
  if (events.length === 0) return null;
  const times = events.flatMap((e) => [Date.parse(e.timestamp), ...(e.uncertainty ? [Date.parse(e.uncertainty.window_start_utc), Date.parse(e.uncertainty.window_end_utc)] : [])]);
  // True to scale within clusters; quiet gaps longer than GAP_MS are compressed to a marked break
  // so a long lull does not squeeze the events that matter.
  const GAP_MS = 15 * 60_000;
  const BREAK_PX = 56;
  const W = 1000;
  const sorted = [...new Set(times)].sort((a, b) => a - b);
  const breaks: { from: number; to: number }[] = [];
  for (let i = 1; i < sorted.length; i++) if (sorted[i]! - sorted[i - 1]! > GAP_MS) breaks.push({ from: sorted[i - 1]!, to: sorted[i]! });
  const t0 = sorted[0]!;
  const realSpan = Math.max(sorted[sorted.length - 1]! - t0 - breaks.reduce((sum, b) => sum + (b.to - b.from), 0), 60_000);
  const pxPerMs = (W - 80 - breaks.length * BREAK_PX) / realSpan;
  const x = (t: number): number => {
    let px = 40;
    let cursor = t0;
    for (const b of breaks) {
      if (t <= b.from) break;
      px += (b.from - cursor) * pxPerMs + BREAK_PX;
      cursor = b.to;
      if (t < b.to) return px - BREAK_PX / 2;
    }
    return px + (t - cursor) * pxPerMs;
  };
  // Alternate labels above and below the axis so close events never overlap.
  const lane = (i: number): { stem: number; time: number; source: number } =>
    i % 2 === 0 ? { stem: 52, time: 46, source: 32 } : { stem: 88, time: 102, source: 116 };

  return (
    <Panel title="Sequence" meta={`${events.length} events · IST${breaks.length ? ' · long gaps compressed' : ' · true to scale'}`} actions={<SectionLink to={caseHref(caseId, 'timeline')}>Timeline</SectionLink>} className="xl:col-span-8">
      <div className="overflow-x-auto bg-ink-950">
        <svg viewBox={`0 0 ${W} 140`} className="block w-full min-w-[40rem]" role="group" aria-label="Event sequence on a true-to-scale time axis">
          <line x1="40" y1="70" x2={W - 40} y2="70" stroke="#3A3A41" />
          {breaks.map((b) => {
            const bx = x(b.from) + BREAK_PX / 2;
            return (
              <g key={b.from} aria-hidden="true">
                <rect x={bx - BREAK_PX / 2 + 6} y="66" width={BREAK_PX - 12} height="8" fill="#0b0a09" />
                <path d={`M ${bx - 6} 76 l 5 -12 M ${bx + 1} 76 l 5 -12`} stroke="#71717A" />
                <text x={bx} y="58" textAnchor="middle" fontFamily={MONO} fontSize="9" fill="#71717A">
                  {Math.round((b.to - b.from) / 60_000)} min
                </text>
              </g>
            );
          })}
          {events.map((event, i) => {
            const cx = x(Date.parse(event.timestamp));
            const l = lane(i);
            const conflict = conflicted.has(event.event_id);
            const tone = conflict ? '#E5243B' : event.ordering_status === 'CONCURRENT_UNCERTAIN' ? '#A1A1AA' : '#EDEDEF';
            return (
              <SvgLink key={event.event_id} href={caseHref(caseId, 'timeline', { event: event.event_id })} label={`${formatEventTime(event.timestamp, event.timestamp_precision)} ${event.title}`}>
                {event.uncertainty && (
                  <rect
                    x={x(Date.parse(event.uncertainty.window_start_utc))}
                    y="64"
                    width={Math.max(2, x(Date.parse(event.uncertainty.window_end_utc)) - x(Date.parse(event.uncertainty.window_start_utc)))}
                    height="12"
                    fill={tone}
                    fillOpacity="0.1"
                  />
                )}
                <line x1={cx} y1="70" x2={cx} y2={l.stem} stroke={tone} strokeOpacity="0.5" />
                <rect className="focus-ring" x={cx - 8} y="62" width="16" height="16" fill="none" stroke="#C8CAD0" strokeDasharray="2 2" opacity="0" />
                <rect x={cx - 5} y="65" width="10" height="10" transform={`rotate(45 ${cx} 70)`} fill={conflict ? '#E5243B' : '#111113'} stroke={tone} strokeWidth="1.5" />
                <text x={cx} y={l.time} textAnchor="middle" fontFamily={MONO} fontSize="11" fill={tone}>
                  {formatEventTime(event.timestamp, event.timestamp_precision)}
                </text>
                <text x={cx} y={l.source} textAnchor="middle" fontFamily={MONO} fontSize="9" fill="#71717A">
                  {event.source_evidence_id}
                </text>
              </SvgLink>
            );
          })}
        </svg>
      </div>
    </Panel>
  );
};

/* ---------------------------------------------------------------- Overview */

export const CaseOverviewSection: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const caseId = detail.case.case_id;
  const synthetic = useIsSynthetic(detail.case);
  const openItems = detail.review_items.filter((item) => item.status !== 'verified');
  const conflicting = openItems.filter((item) => item.category === 'conflicting' && item.records.length >= 2);
  const others = openItems.filter((item) => !conflicting.includes(item));


  const lead = conflicting[0] ?? openItems[0];

  return (
    <div className="grid grid-cols-1 items-start gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
      <SummaryHero detail={detail} openReview={openItems.length} lead={lead} />

      <TimelineStrip detail={detail} />

      <Panel title="Review queue" meta={`${openItems.length} open`} actions={<SectionLink to={caseHref(caseId, 'contradictions')}>Review</SectionLink>} className="xl:col-span-4">
        <ul className="divide-y divide-ink-line">
          {[...conflicting, ...others].map((item) => (
            <li key={item.id}>
              <Link to={caseHref(caseId, 'contradictions', { item: item.id })} className="flex items-center justify-between gap-3 px-4 py-3 transition-colors hover:bg-white/[0.02]">
                <span className="min-w-0">
                  <span className="block truncate text-sm text-fg">{item.title}</span>
                  <span className="mt-1 flex items-center gap-2">
                    <ReviewCategoryBadge category={item.category} />
                    <span className="font-mono text-micro text-fg-subtle">{item.id}</span>
                  </span>
                </span>
                <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />
              </Link>
            </li>
          ))}
        </ul>
      </Panel>

      <Panel title="Evidence" meta={`${detail.evidence.length} artifacts`} actions={<SectionLink to={caseHref(caseId, 'evidence')}>Intake</SectionLink>} className="xl:col-span-7">
        <ul className="grid grid-cols-2 gap-3 p-3 sm:grid-cols-3 sm:p-4">
          {detail.evidence.map((evidence) => (
            <li key={evidence.evidence_id}>
              <Link
                to={caseHref(caseId, 'evidence', { evidence: evidence.evidence_id })}
                className="group block border border-ink-line bg-ink-950 transition-[border-color,transform] duration-base hover:border-ink-strong motion-safe:hover:-translate-y-0.5"
              >
                <EvidencePreview evidence={evidence} synthetic={synthetic} className="border-b border-ink-line" />
                <span className="block px-2.5 py-2">
                  <span className={`block font-mono text-micro ${evidence.review === 'flagged' ? 'text-crimson' : 'text-fg-subtle'}`}>
                    {evidence.evidence_id} · {evidence.source_type ?? 'Unclassified'}
                  </span>
                  <span className="block truncate text-xs text-fg">{evidence.original_filename}</span>
                </span>
              </Link>
            </li>
          ))}
        </ul>
      </Panel>

      <GraphPreview detail={detail} />
    </div>
  );
};
