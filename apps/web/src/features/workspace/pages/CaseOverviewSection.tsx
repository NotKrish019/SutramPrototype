import React from 'react';
import { ArrowRight } from 'lucide-react';
import { ProvenanceBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { Panel, ReviewCategoryBadge } from '../components/ui';
import { formatEventTime } from '../format';
import type { CaseDetail } from '../types';

const SectionLink: React.FC<{ to: string; children: React.ReactNode }> = ({ to, children }) => (
  <Link to={to} className="inline-flex items-center gap-1 font-mono text-micro uppercase text-fg-muted hover:text-signal-amber">
    {children}
    <ArrowRight aria-hidden="true" className="h-3 w-3" />
  </Link>
);

export const CaseOverviewSection: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const caseId = detail.case.case_id;
  const openItems = detail.review_items.filter((item) => item.status !== 'verified');
  const typeCounts = detail.evidence.reduce<Record<string, number>>((acc, e) => {
    const key = e.source_type ?? 'Unclassified';
    acc[key] = (acc[key] ?? 0) + 1;
    return acc;
  }, {});
  const summary = [
    { label: 'Evidence collected', value: detail.evidence.length, href: caseHref(caseId, 'evidence') },
    { label: 'Entities identified', value: detail.entities.length, href: caseHref(caseId, 'graph') },
    { label: 'Events reconstructed', value: detail.events.length, href: caseHref(caseId, 'timeline') },
    { label: 'Needs investigator review', value: openItems.length, href: caseHref(caseId, 'contradictions'), alert: true },
  ];

  return (
    <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
      <section aria-labelledby="case-summary-heading" className="border border-ink-line bg-ink-900 xl:col-span-12">
        <h2 id="case-summary-heading" className="sr-only">Investigation summary</h2>
        <div className="grid lg:grid-cols-12">
          <p className="border-b border-ink-line px-5 py-5 text-sm leading-relaxed text-fg-muted lg:col-span-5 lg:border-b-0 lg:border-r">
            {detail.case.description}
          </p>
          <dl className="grid grid-cols-2 sm:grid-cols-4 lg:col-span-7">
            {summary.map((item, index) => (
              <div key={item.label} className={`border-ink-line px-5 py-5 ${index < 3 ? 'sm:border-r' : ''} ${index % 2 === 0 ? 'max-sm:border-r' : ''} ${index < 2 ? 'max-sm:border-b' : ''}`}>
                <dt className="min-h-[2.75rem] font-mono text-micro uppercase text-fg-subtle">{item.label}</dt>
                <dd className={`mt-2 font-display text-4xl font-semibold leading-none ${item.alert ? 'text-signal-flag' : 'text-fg'}`}>
                  {String(item.value).padStart(2, '0')}
                </dd>
                <dd className="mt-2">
                  <SectionLink to={item.href}>View</SectionLink>
                </dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      <Panel title="Event sequence" meta={`${detail.events.length} events`} actions={<SectionLink to={caseHref(caseId, 'timeline')}>Timeline</SectionLink>} className="xl:col-span-7">
        <ol className="divide-y divide-ink-line">
          {[...detail.events]
            .sort((a, b) => a.timestamp.localeCompare(b.timestamp))
            .map((event) => (
              <li key={event.event_id}>
                <Link
                  to={caseHref(caseId, 'timeline', { event: event.event_id })}
                  className="grid grid-cols-[5.5rem_1fr_auto] items-center gap-3 px-4 py-2.5 transition-colors hover:bg-white/[0.02]"
                >
                  <time dateTime={event.timestamp} className="font-mono text-xs text-fg-muted">
                    {formatEventTime(event.timestamp, event.timestamp_precision)}
                  </time>
                  <span className="min-w-0 truncate text-sm text-fg">
                    {event.title}
                    {event.uncertainty && event.timestamp_precision === 'EXACT_SECOND' && (
                      <span className="ml-2 font-mono text-micro uppercase text-signal-amber">clock uncertain</span>
                    )}
                  </span>
                  <span className="font-mono text-micro text-fg-subtle">{event.source_evidence_id}</span>
                </Link>
              </li>
            ))}
        </ol>
      </Panel>

      <Panel title="Review queue" meta={`${openItems.length} open`} actions={<SectionLink to={caseHref(caseId, 'contradictions')}>Review</SectionLink>} className="xl:col-span-5">
        <ul className="divide-y divide-ink-line">
          {openItems.map((item) => (
            <li key={item.id}>
              <Link to={caseHref(caseId, 'contradictions', { item: item.id })} className="block px-4 py-3 transition-colors hover:bg-white/[0.02]">
                <span className="flex items-center gap-2">
                  <ReviewCategoryBadge category={item.category} />
                  <span className="font-mono text-micro text-fg-subtle">{item.id}</span>
                </span>
                <span className="mt-1.5 block text-sm text-fg">{item.title}</span>
              </Link>
            </li>
          ))}
        </ul>
      </Panel>

      <Panel title="Evidence by type" actions={<SectionLink to={caseHref(caseId, 'evidence')}>Repository</SectionLink>} className="xl:col-span-5">
        <ul className="space-y-3 px-4 py-4">
          {Object.entries(typeCounts).map(([type, count]) => (
            <li key={type}>
              <div className="flex justify-between font-mono text-micro uppercase">
                <span className={type === 'Unclassified' ? 'text-signal-amber' : 'text-fg-muted'}>{type}</span>
                <span className="text-fg">{count}</span>
              </div>
              <div className="mt-1.5 h-1 bg-ink-700">
                <div className={`h-full ${type === 'Unclassified' ? 'bg-signal-amber/60' : 'bg-fg/70'}`} style={{ width: `${(count / detail.evidence.length) * 100}%` }} />
              </div>
            </li>
          ))}
        </ul>
        {typeCounts.Unclassified && (
          <p className="border-t border-ink-line px-4 py-2.5 text-xs text-fg-subtle">
            Unclassified: access-control and network logs have no canonical <code className="font-mono">EvidenceType</code> yet.
          </p>
        )}
      </Panel>

      <Panel title="Key entities" actions={<SectionLink to={caseHref(caseId, 'graph')}>Graph</SectionLink>} className="xl:col-span-7">
        <ul className="grid divide-y divide-ink-line sm:grid-cols-2 sm:divide-y-0">
          {detail.entities.map((entity) => {
            const links = detail.relationships.filter((r) => r.source_id === entity.entity_id || r.target_id === entity.entity_id);
            const inferred = links.some((r) => r.provenance === 'inferred');
            return (
              <li key={entity.entity_id} className="sm:border-b sm:border-ink-line sm:[&:nth-child(odd)]:border-r">
                <Link to={caseHref(caseId, 'graph', { node: entity.entity_id })} className="flex items-center justify-between gap-3 px-4 py-3 transition-colors hover:bg-white/[0.02]">
                  <span className="min-w-0">
                    <span className="block truncate text-sm text-fg">{entity.canonical_name}</span>
                    <span className="block truncate font-mono text-micro text-fg-subtle">{entity.type_label}</span>
                  </span>
                  <span className="flex shrink-0 items-center gap-2">
                    <span className="font-mono text-micro text-fg-subtle">{links.length} links</span>
                    {inferred && <ProvenanceBadge value="inferred" />}
                  </span>
                </Link>
              </li>
            );
          })}
        </ul>
      </Panel>
    </div>
  );
};
