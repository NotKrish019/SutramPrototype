import React, { useState } from 'react';
import { FileDown, Lock, Printer } from 'lucide-react';
import { useWorkspaceRepository } from '../data/WorkspaceDataContext';
import { ProvenanceBadge } from '../../../components/ui/primitives';
import { CaseStatusBadge, FilterChip, Panel, WsButton } from '../components/ui';
import { formatDateTime, formatEventTime, formatUtc, REVIEW_CATEGORY_LABEL, shortHash } from '../format';
import type { CaseDetail } from '../types';

type ReportSection = 'summary' | 'inventory' | 'entities' | 'timeline' | 'review' | 'provenance';

const SECTIONS: { id: ReportSection; label: string }[] = [
  { id: 'summary', label: 'Case summary' },
  { id: 'inventory', label: 'Evidence inventory' },
  { id: 'entities', label: 'Entities & relationships' },
  { id: 'timeline', label: 'Timeline reconstruction' },
  { id: 'review', label: 'Contradiction review' },
  { id: 'provenance', label: 'Evidence provenance' },
];

const ReportHeading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <h3 className="mb-3 border-b border-ink-line pb-2 font-mono text-micro uppercase text-fg-subtle">{children}</h3>
);

/** Report preview assembled in the browser from the loaded case records. */
export const ReportsWorkspace: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const synthetic = useWorkspaceRepository().source === 'synthetic';
  const openReview = detail.review_items.filter((item) => item.status !== 'verified');
  const conflicting = openReview.filter((item) => item.category === 'conflicting').length;
  const exportReasons = [
    ...(conflicting > 0 ? [`${conflicting} unresolved contradiction${conflicting === 1 ? '' : 's'}`] : []),
    ...(openReview.length > 0 ? [`${openReview.length} open review item${openReview.length === 1 ? '' : 's'}`] : []),
    'Chain of custody not finalised in this workspace',
  ];
  const [included, setIncluded] = useState<Set<ReportSection>>(() => new Set(SECTIONS.map((s) => s.id)));
  const toggle = (id: ReportSection): void =>
    setIncluded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const c = detail.case;
  const nameOf = (id: string): string =>
    detail.entities.find((e) => e.entity_id === id)?.canonical_name ?? detail.events.find((e) => e.event_id === id)?.title ?? id;

  return (
    <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
      <div className="min-w-0 space-y-4 xl:col-span-4 xl:self-start print:hidden">
        <Panel title="Report contents">
          <div className="flex flex-wrap gap-1.5 p-3" role="group" aria-label="Report sections">
            {SECTIONS.map((section) => (
              <FilterChip key={section.id} active={included.has(section.id)} onClick={() => toggle(section.id)}>
                {section.label}
              </FilterChip>
            ))}
          </div>
          <div className="flex flex-col gap-2 border-t border-ink-line p-3">
            <WsButton icon={Printer} onClick={() => window.print()}>
              Print preview
            </WsButton>
            <WsButton icon={FileDown} disabled aria-describedby="report-export-note">
              Export PDF / JSON
            </WsButton>
            <div className="mt-1 border border-signal-flag/50 bg-signal-flag/[0.06] p-3">
              <p className="flex items-center gap-2 font-mono text-micro uppercase text-signal-flag">
                <Lock aria-hidden="true" className="h-3.5 w-3.5" /> SHIP_EXPORT_BLOCKED
              </p>
              <ul className="mt-2 space-y-1 text-xs text-fg-muted">
                {exportReasons.map((reason) => (
                  <li key={reason} className="flex gap-2">
                    <span aria-hidden="true" className="mt-1.5 h-1 w-1 shrink-0 bg-signal-flag" />
                    {reason}
                  </li>
                ))}
              </ul>
            </div>
            <p id="report-export-note" className="text-xs leading-relaxed text-fg-subtle">
              Judicial export status: <strong>SHIP_EXPORT_BLOCKED</strong>. Dossier export is blocked pending verification of unresolved contradiction leads and chain of custody finalization. The preview is assembled in the browser from the records shown in this workspace.
            </p>
          </div>
        </Panel>
        <Panel title="Investigation notes">
          <p className="px-4 py-4 text-sm leading-relaxed text-fg-muted">
            Notes need persistent, attributable storage, which requires a case API and an investigator identity model.
            Neither exists yet, so notes are not offered rather than kept only in this browser.
          </p>
        </Panel>
      </div>

      <article aria-label="Report preview" className="min-w-0 border border-ink-line bg-ink-900 xl:col-span-8">
        <header className="border-b border-ink-line px-6 py-6 sm:px-8">
          <p className="font-mono text-micro uppercase text-signal-accent">
            Draft report preview · {synthetic ? 'synthetic demonstration data' : 'live reconstruction data'}
          </p>
          <h2 className="mt-3 font-serif text-3xl leading-none text-fg-bright">{c.title}</h2>
          <div className="mt-3 flex flex-wrap items-center gap-3 font-mono text-micro uppercase text-fg-subtle">
            <span>{c.case_id}</span>
            {c.fir_number && <span>{c.fir_number}</span>}
            <CaseStatusBadge status={c.status} />
            <span>Generated {formatDateTime(new Date().toISOString())} (browser)</span>
          </div>
          <ul aria-label="How to read this report" className="mt-5 flex flex-wrap gap-x-5 gap-y-2 border-t border-ink-line pt-4 text-xs text-fg-muted">
            <li className="flex items-center gap-2"><span aria-hidden="true" className="h-px w-5 bg-fg" /><span className="text-fg">Evidence</span> observed records, cited by exhibit</li>
            <li className="flex items-center gap-2"><span aria-hidden="true" className="h-px w-5 border-t border-dashed border-paper" /><span className="text-fg">Inference</span> derived links, never proof</li>
            <li className="flex items-center gap-2"><span aria-hidden="true" className="h-1.5 w-1.5 bg-signal-flag" /><span className="text-fg">Unresolved</span> open questions, stated not hidden</li>
          </ul>
        </header>
        <div className="space-y-8 px-6 py-6 sm:px-8">
          {included.size === 0 && <p className="text-sm text-fg-subtle">Select at least one section.</p>}

          {included.has('summary') && (
            <section>
              <ReportHeading>Case summary</ReportHeading>
              <p className="text-sm leading-relaxed text-fg">{c.description}</p>
              <p className="mt-3 font-mono text-xs text-fg-muted">
                {detail.evidence.length} artifacts · {detail.entities.length} entities · {detail.events.length} events ·{' '}
                {detail.review_items.filter((i) => i.status !== 'verified').length} open review items
              </p>
            </section>
          )}

          {included.has('inventory') && (
            <section>
              <ReportHeading>Evidence inventory</ReportHeading>
              <div className="overflow-x-auto">
                <table className="w-full min-w-[34rem] text-left text-sm">
                  <thead>
                    <tr className="font-mono text-micro uppercase text-fg-subtle">
                      <th scope="col" className="py-1.5 pr-4 font-normal">ID</th>
                      <th scope="col" className="py-1.5 pr-4 font-normal">File</th>
                      <th scope="col" className="py-1.5 pr-4 font-normal">Type</th>
                      <th scope="col" className="py-1.5 font-normal">SHA-256</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-ink-line">
                    {detail.evidence.map((e) => (
                      <tr key={e.evidence_id}>
                        <td className="py-2 pr-4 font-mono text-xs text-fg">{e.evidence_id}</td>
                        <td className="py-2 pr-4 text-fg">{e.original_filename}</td>
                        <td className="py-2 pr-4 font-mono text-xs text-fg-muted">{e.source_type ?? 'Unclassified'}</td>
                        <td className="py-2 font-mono text-xs text-fg-muted">{shortHash(e.sha256_hash)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          )}

          {included.has('entities') && (
            <section>
              <ReportHeading>Entities &amp; relationships</ReportHeading>
              <ul className="space-y-2 text-sm">
                {detail.relationships.map((r) => (
                  <li key={r.relationship_id} className="flex flex-wrap items-center gap-2">
                    <span className="text-fg">{nameOf(r.source_id)} — {r.label} → {nameOf(r.target_id)}</span>
                    <ProvenanceBadge value={r.provenance} />
                    <span className="font-mono text-micro text-fg-subtle">basis: {r.evidence_basis.join(', ') || 'external record'}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {included.has('timeline') && (
            <section>
              <ReportHeading>Timeline reconstruction (IST)</ReportHeading>
              <ol className="space-y-1.5 text-sm">
                {[...detail.events].sort((a, b) => a.timestamp.localeCompare(b.timestamp)).map((e) => (
                  <li key={e.event_id} className="grid grid-cols-[5.5rem_1fr] gap-3">
                    <span className="font-mono text-xs text-fg-muted">{formatEventTime(e.timestamp, e.timestamp_precision)}</span>
                    <span className="text-fg">
                      {e.title} <span className="font-mono text-micro text-fg-subtle">[{e.source_evidence_id} · {e.timestamp_precision}{e.ordering_status === 'CONCURRENT_UNCERTAIN' ? ' · order uncertain' : ''}]</span>
                    </span>
                  </li>
                ))}
              </ol>
            </section>
          )}

          {included.has('review') && (
            <section>
              <ReportHeading>Contradiction review</ReportHeading>
              <ul className="space-y-3 text-sm">
                {detail.review_items.map((item) => (
                  <li key={item.id}>
                    <p className="text-fg"><span className="font-mono text-xs text-fg-muted">{item.id} · {REVIEW_CATEGORY_LABEL[item.category]}</span> — {item.title}</p>
                    <p className="text-fg-muted">{item.suggested_action}</p>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {included.has('provenance') && (
            <section>
              <ReportHeading>Evidence provenance</ReportHeading>
              <ul className="space-y-2 text-sm">
                {detail.evidence.map((e) => (
                  <li key={e.evidence_id}>
                    <span className="font-mono text-xs text-fg">{e.evidence_id}</span>{' '}
                    <span className="text-fg-muted">ingested {formatUtc(e.ingested_at)} · digest {shortHash(e.sha256_hash)} recorded at ingestion</span>
                  </li>
                ))}
              </ul>
              <p className="mt-3 text-xs text-fg-subtle">
                Digests support detection of changes after ingestion; they do not by themselves establish authenticity before acquisition.
              </p>
            </section>
          )}
        </div>
      </article>
    </div>
  );
};
