import React, { useMemo, useState } from 'react';
import { ArrowRight, Info, Scale } from 'lucide-react';
import { ReviewBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { useCaseIndex, useQuerySelection } from '../caseIndex';
import { EmptyState, FilterChip, Panel, ReviewCategoryBadge } from '../components/ui';
import { REVIEW_CATEGORY_LABEL } from '../format';
import type { CaseDetail, ReviewCategory, ReviewItem } from '../types';

const CATEGORIES: ReviewCategory[] = ['conflicting', 'uncertain', 'missing', 'unverified', 'supporting'];

const CATEGORY_HELP: Record<ReviewCategory, string> = {
  conflicting: 'Two sources make claims that cannot both be true as recorded.',
  uncertain: 'A proposed relationship that rests on inference, not documentation.',
  missing: 'Expected coverage or records that are absent.',
  unverified: 'Information that has not been corroborated by an independent source.',
  supporting: 'Independent records that agree with each other.',
};

const ReviewDetail: React.FC<{ item: ReviewItem; detail: CaseDetail }> = ({ item, detail }) => {
  const index = useCaseIndex(detail);
  const caseId = detail.case.case_id;
  return (
    <Panel as="aside" title={`${item.id} · review item`}>
      <div className="border-b border-ink-line px-5 py-5">
        <div className="flex flex-wrap items-center gap-2">
          <ReviewCategoryBadge category={item.category} />
          <ReviewBadge value={item.status} />
        </div>
        <h2 className="mt-3 font-display text-2xl font-semibold uppercase leading-tight text-fg">{item.title}</h2>
        <p className="mt-2 text-sm leading-relaxed text-fg-muted">{item.summary}</p>
      </div>

      <h3 className="border-b border-ink-line bg-ink-850 px-5 py-2 font-mono text-micro uppercase text-fg-subtle">
        Observed records
      </h3>
      <div className={`grid gap-px bg-ink-line ${item.records.length > 1 ? 'sm:grid-cols-2' : ''}`}>
        {item.records.map((record, i) => {
          const evidence = index.evidence.get(record.evidence_id);
          return (
            <div key={`${record.evidence_id}-${i}`} className="bg-ink-950 p-5">
              <p className="font-mono text-micro uppercase text-fg-subtle">
                Source {String.fromCharCode(65 + i)} · {record.evidence_id}
              </p>
              <p className={`mt-3 font-display text-3xl font-semibold leading-none ${item.category === 'conflicting' ? 'text-fg' : 'text-fg'}`}>{record.value}</p>
              <p className="mt-2 text-sm text-fg-muted">{record.label}</p>
              <Link to={caseHref(caseId, 'evidence', { evidence: record.evidence_id })} className="mt-3 inline-flex max-w-full items-center gap-1 truncate font-mono text-micro text-fg-subtle hover:text-signal-amber">
                {evidence?.original_filename} <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0" />
              </Link>
            </div>
          );
        })}
      </div>
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
                <Link to={caseHref(caseId, 'timeline', { event: id })} className="text-sm text-fg hover:text-signal-amber">
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
                <Link to={caseHref(caseId, 'graph', { node: id })} className="text-sm text-fg hover:text-signal-amber">
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
      <div className="flex items-start gap-3 border border-ink-line bg-ink-900 px-4 py-3 text-sm text-fg-muted">
        <Scale aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-signal-amber" />
        <p>
          Contradiction and inconsistency review. Member 4&apos;s contradiction engine exists in the backend but is not yet exposed
          through an API, so the items below are part of the synthetic case. Nothing here is resolved automatically.
        </p>
      </div>

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
                      onClick={() => setSelectedId(item.id)}
                      className={`relative w-full px-4 py-3.5 text-left transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-amber ${
                        isSelected ? 'bg-white/[0.04]' : 'hover:bg-white/[0.02]'
                      }`}
                    >
                      <span aria-hidden="true" className={`absolute inset-y-0 left-0 w-0.5 ${isSelected ? 'bg-signal-amber' : 'bg-transparent'}`} />
                      <span className="flex flex-wrap items-center gap-2">
                        <ReviewCategoryBadge category={item.category} />
                        <span className="font-mono text-micro text-fg-subtle">{item.id}</span>
                        <span className="ml-auto"><ReviewBadge value={item.status} /></span>
                      </span>
                      <span className="mt-2 block text-sm font-medium text-fg">{item.title}</span>
                      <span className="mt-1 block text-xs text-fg-subtle">{CATEGORY_HELP[item.category]}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </Panel>
        <div className="xl:col-span-7">{selected && <ReviewDetail item={selected} detail={detail} />}</div>
      </div>
    </div>
  );
};
