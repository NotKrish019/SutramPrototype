import React, { useMemo, useState } from 'react';
import { ArrowRight, Info, Scale } from 'lucide-react';
import { ReviewBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { useCaseIndex, useQuerySelection } from '../caseIndex';
import { EmptyState, FilterChip, Panel, ReviewCategoryBadge } from '../components/ui';
import { REVIEW_CATEGORY_LABEL } from '../format';
import type { CaseDetail, ReviewCategory, ReviewItem, ReviewRecord } from '../types';
import { EvidencePreview } from '../components/EvidencePreview';
import { useWorkspaceRepository } from '../data/WorkspaceDataContext';

const CATEGORIES: ReviewCategory[] = ['conflicting', 'uncertain', 'missing', 'unverified', 'supporting'];

const CATEGORY_HELP: Record<ReviewCategory, string> = {
  conflicting: 'Two sources make claims that cannot both be true as recorded.',
  uncertain: 'A proposed relationship that rests on inference, not documentation.',
  missing: 'Expected coverage or records that are absent.',
  unverified: 'Information that has not been corroborated by an independent source.',
  supporting: 'Independent records that agree with each other.',
};

/** Seconds since midnight for a recorded "HH:MM:SS" value; null when the value is not a clock time. */
function clockSeconds(value: string): number | null {
  const match = /(\d{2}):(\d{2}):(\d{2})/.exec(value);
  return match ? Number(match[1]) * 3600 + Number(match[2]) * 60 + Number(match[3]) : null;
}

/**
 * Both recorded clock values on one axis, so the size of the disagreement is visible at a glance.
 * Values are plotted exactly as recorded by each source; no clock correction is applied.
 */
const ClockComparison: React.FC<{ records: ReviewRecord[] }> = ({ records }) => {
  const points = records.map((r, i) => ({ id: r.evidence_id, label: String.fromCharCode(65 + i), value: r.value, s: clockSeconds(r.value) }));
  if (points.length !== 2 || points.some((p) => p.s === null)) return null;
  const [a, b] = points as [{ id: string; label: string; value: string; s: number }, { id: string; label: string; value: string; s: number }];
  const lo = Math.min(a.s, b.s);
  const hi = Math.max(a.s, b.s);
  const pad = Math.max(30, (hi - lo) * 0.35);
  const x = (sec: number): number => 40 + ((sec - (lo - pad)) / (hi - lo + 2 * pad)) * 520;
  const delta = hi - lo;
  return (
    <div className="border-t border-ink-line bg-ink-950 px-5 py-4">
      <svg viewBox="0 0 600 70" className="block w-full" role="img" aria-label={`Source ${a.label} at ${a.value} and source ${b.label} at ${b.value}, ${Math.floor(delta / 60)} min ${delta % 60} s apart as recorded`}>
        <line x1="40" y1="40" x2="560" y2="40" stroke="#3A342D" />
        <rect x={x(lo)} y="36" width={x(hi) - x(lo)} height="8" fill="#E32636" fillOpacity="0.18" />
        <path d={`M ${x(lo)} 24 V 30 H ${x(hi)} V 24`} fill="none" stroke="#E32636" pathLength={1} className="ws-draw" />
        <text x={(x(lo) + x(hi)) / 2} y="18" textAnchor="middle" fontFamily="JetBrains Mono, monospace" fontSize="11" fill="#E32636">
          Δ {Math.floor(delta / 60)} min {delta % 60} s
        </text>
        {[a, b].map((p) => (
          <g key={p.id}>
            <circle cx={x(p.s)} cy="40" r="5" fill="#080807" stroke="#F1E9DB" strokeWidth="1.5" />
            <text x={x(p.s)} y="62" textAnchor="middle" fontFamily="JetBrains Mono, monospace" fontSize="10" fill="#A9A197">
              {p.label} · {p.id}
            </text>
          </g>
        ))}
      </svg>
    </div>
  );
};

const ReviewDetail: React.FC<{ item: ReviewItem; detail: CaseDetail }> = ({ item, detail }) => {
  const index = useCaseIndex(detail);
  const caseId = detail.case.case_id;
  const synthetic = useWorkspaceRepository().source === 'synthetic';
  const conflict = item.category === 'conflicting';
  return (
    <Panel as="aside" title={`${item.id} · review item`} className="ws-inspector-enter">
      <div className="border-b border-ink-line px-5 py-5">
        <div className="flex flex-wrap items-center gap-2">
          <ReviewCategoryBadge category={item.category} />
          <ReviewBadge value={item.status} />
        </div>
        <h2 className="mt-3 font-serif text-[1.75rem] font-normal leading-tight text-fg-bright">{item.title}</h2>
        <p className="mt-2 text-sm leading-relaxed text-fg-muted">{item.summary}</p>
      </div>

      <h3 className="border-b border-ink-line bg-ink-850 px-5 py-2 font-mono text-micro uppercase text-fg-subtle">
        Observed records
      </h3>
      <div className="relative">
        <div className={`grid gap-px bg-ink-line ${item.records.length > 1 ? 'sm:grid-cols-2' : ''}`}>
          {item.records.map((record, i) => {
            const evidence = index.evidence.get(record.evidence_id);
            return (
              <div key={`${record.evidence_id}-${i}`} className={`bg-ink-950 p-5 ${conflict ? 'border-t-2 border-crimson/70' : ''}`}>
                <p className="font-mono text-micro uppercase text-fg-subtle">
                  Source {String.fromCharCode(65 + i)} · {record.evidence_id}
                </p>
                <p className="mt-3 font-display text-3xl font-semibold leading-none text-fg-bright">{record.value}</p>
                <p className="mt-2 text-sm text-fg-muted">{record.label}</p>
                {evidence && (
                  <Link to={caseHref(caseId, 'evidence', { evidence: record.evidence_id })} className="group mt-4 flex items-center gap-3">
                    <EvidencePreview evidence={evidence} synthetic={synthetic} className="w-28 shrink-0 border border-ink-line transition-colors group-hover:border-ink-strong" />
                    <span className="flex min-w-0 items-center gap-1 truncate font-mono text-micro text-fg-subtle group-hover:text-signal-accent">
                      {evidence.original_filename} <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0" />
                    </span>
                  </Link>
                )}
              </div>
            );
          })}
        </div>
        {conflict && item.records.length === 2 && (
          <span aria-hidden="true" className="absolute left-1/2 top-1/2 hidden h-9 w-9 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-crimson font-serif text-lg text-white shadow-[0_0_0_6px_#080807] sm:grid">
            ≠
          </span>
        )}
      </div>
      {conflict && <ClockComparison records={item.records} />}
      {item.discrepancy && (
        <p className="border-y border-signal-flag/30 bg-signal-flag/[0.06] px-5 py-3 font-mono text-xs text-signal-flag">
          Nature of discrepancy · {item.discrepancy}
        </p>
      )}

      <div className="grid gap-px bg-ink-line sm:grid-cols-2">
        <div className="bg-ink-900 px-5 py-4">
          <p className="font-mono text-micro uppercase text-fg-subtle">Related events</p>
          <ul className="mt-2 space-y-1.5">
            {item.related_event_ids.map((id) => (
              <li key={id}>
                <Link to={caseHref(caseId, 'timeline', { event: id })} className="text-sm text-fg hover:text-signal-accent">
                  {index.nameOf(id)} <span className="font-mono text-micro text-fg-subtle">{id}</span>
                </Link>
              </li>
            ))}
          </ul>
        </div>
        <div className="bg-ink-900 px-5 py-4">
          <p className="font-mono text-micro uppercase text-fg-subtle">Related entities</p>
          <ul className="mt-2 space-y-1.5">
            {item.related_entity_ids.map((id) => (
              <li key={id}>
                <Link to={caseHref(caseId, 'graph', { node: id })} className="text-sm text-fg hover:text-signal-accent">
                  {index.nameOf(id)}
                </Link>
              </li>
            ))}
          </ul>
        </div>
      </div>

      <div className="border-t border-ink-line px-5 py-4">
        <p className="font-mono text-micro uppercase text-fg-subtle">Suggested verification</p>
        <p className="mt-1.5 text-sm leading-relaxed text-fg">{item.suggested_action}</p>
      </div>
      <div className="border-t border-ink-line px-5 py-4">
        <p className="font-mono text-micro uppercase text-fg-subtle">Limitations &amp; alternative explanations</p>
        <ul className="mt-2 list-disc space-y-1 pl-5 text-sm leading-relaxed text-fg-muted marker:text-fg-subtle">
          {item.limitations.map((limitation) => (
            <li key={limitation}>{limitation}</li>
          ))}
        </ul>
      </div>
      <p className="flex items-start gap-2 border-t border-ink-line px-5 py-3 text-xs leading-relaxed text-fg-subtle">
        <Info aria-hidden="true" className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        A review item marks something to examine. It is not evidence of wrongdoing and does not resolve which source is correct.
      </p>
    </Panel>
  );
};

export const ContradictionReview: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const [selectedId, setSelectedId] = useQuerySelection('item');
  const [category, setCategory] = useState<ReviewCategory | 'all'>('all');
  const items = useMemo(
    () => detail.review_items.filter((item) => category === 'all' || item.category === category),
    [detail.review_items, category],
  );
  const selected = detail.review_items.find((item) => item.id === selectedId) ?? items[0] ?? null;
  const count = (value: ReviewCategory | 'all'): number =>
    value === 'all' ? detail.review_items.length : detail.review_items.filter((item) => item.category === value).length;

  return (
    <div className="space-y-4 p-4 sm:p-6 lg:p-8">
      <p className="flex items-center gap-2 font-mono text-micro uppercase text-fg-subtle">
        <Scale aria-hidden="true" className="h-3.5 w-3.5 shrink-0 text-signal-accent" />
        Synthetic review items · contradiction engine not yet exposed through an API · nothing resolved automatically
      </p>

      <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filter review items by category">
        <FilterChip active={category === 'all'} onClick={() => setCategory('all')} count={count('all')}>All</FilterChip>
        {CATEGORIES.map((value) => (
          <FilterChip key={value} active={category === value} onClick={() => setCategory(value)} count={count(value)}>
            {REVIEW_CATEGORY_LABEL[value]}
          </FilterChip>
        ))}
      </div>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-12">
        <Panel title="Review queue" meta={`${items.length} items`} className="xl:col-span-5 xl:self-start">
          {items.length === 0 ? (
            <EmptyState icon={Scale} title="Nothing in this category" />
          ) : (
            <ul className="divide-y divide-ink-line">
              {items.map((item) => {
                const isSelected = selected?.id === item.id;
                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      aria-pressed={isSelected}
                      title={CATEGORY_HELP[item.category]}
                      onClick={() => setSelectedId(item.id)}
                      className={`relative w-full px-4 py-3.5 text-left transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-accent ${
                        isSelected ? 'bg-white/[0.04]' : 'hover:bg-white/[0.02]'
                      }`}
                    >
                      <span aria-hidden="true" className={`absolute inset-y-0 left-0 w-0.5 ${isSelected ? 'bg-signal-accent' : 'bg-transparent'}`} />
                      <span className="flex flex-wrap items-center gap-2">
                        <ReviewCategoryBadge category={item.category} />
                        <span className="font-mono text-micro text-fg-subtle">{item.id}</span>
                        <span className="ml-auto"><ReviewBadge value={item.status} /></span>
                      </span>
                      <span className="mt-2 block text-sm font-medium text-fg">{item.title}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </Panel>
        <div className="xl:col-span-7">{selected && <ReviewDetail key={selected.id} item={selected} detail={detail} />}</div>
      </div>
    </div>
  );
};
