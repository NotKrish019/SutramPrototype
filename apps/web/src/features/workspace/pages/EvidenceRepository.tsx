import React, { useId, useMemo, useState } from 'react';
import { ArrowRight, Check, ChevronDown, Copy, FileSearch, LayoutGrid, List, Plus, X } from 'lucide-react';
import { ProvenanceBadge, ReviewBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { useCaseIndex, useQuerySelection, type CaseIndex } from '../caseIndex';
import { useWorkspaceRepository } from '../data/WorkspaceDataContext';
import { EvidencePreview } from '../components/EvidencePreview';
import { EmptyState, EvidenceTypeLabel, FilterChip, KeyValueList, Panel, ReviewCategoryBadge, SearchField, SelectField, WsButton } from '../components/ui';
import { formatBytes, formatDateTime, formatEventTime, formatUtc, shortHash } from '../format';
import type { CaseDetail, ReviewStatus, WorkspaceEvidence } from '../types';
import { EvidenceUploadPanel } from './EvidenceUploadPanel';

const REVIEW_FILTERS: { value: ReviewStatus | 'all'; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'flagged', label: 'Flagged' },
  { value: 'pending', label: 'Pending' },
  { value: 'verified', label: 'Reviewed' },
];

const CopyHash: React.FC<{ value: string }> = ({ value }) => {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle');
  const copy = (): void => {
    if (!navigator.clipboard) {
      setState('failed');
      return;
    }
    navigator.clipboard.writeText(value).then(
      () => setState('copied'),
      () => setState('failed'),
    );
  };
  return (
    <button
      type="button"
      onClick={copy}
      className="inline-flex items-center gap-1.5 font-mono text-micro uppercase text-fg-muted hover:text-fg"
      aria-live="polite"
    >
      {state === 'copied' ? <Check aria-hidden="true" className="h-3 w-3 text-signal-ok" /> : <Copy aria-hidden="true" className="h-3 w-3" />}
      {state === 'copied' ? 'Copied' : state === 'failed' ? 'Copy unavailable' : 'Copy SHA-256'}
    </button>
  );
};

const LinkList: React.FC<{ items: { id: string; label: string; detail?: string; href: string }[]; empty: string }> = ({ items, empty }) =>
  items.length === 0 ? (
    <p className="px-4 py-3 text-sm text-fg-subtle">{empty}</p>
  ) : (
    <ul className="divide-y divide-ink-line">
      {items.map((item) => (
        <li key={item.id}>
          <Link to={item.href} className="flex items-center justify-between gap-3 px-4 py-2.5 transition-colors hover:bg-white/[0.02]">
            <span className="min-w-0">
              <span className="block truncate text-sm text-fg">{item.label}</span>
              {item.detail && <span className="block truncate font-mono text-micro text-fg-subtle">{item.detail}</span>}
            </span>
            <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />
          </Link>
        </li>
      ))}
    </ul>
  );

const SubHeading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle">{children}</h3>
);

export const EvidenceDetailPanel: React.FC<{
  evidence: WorkspaceEvidence;
  detail: CaseDetail;
  index: CaseIndex;
  onSelect: (id: string) => void;
  onClose: () => void;
}> = ({ evidence, detail, index, onSelect, onClose }) => {
  const synthetic = useWorkspaceRepository().source === 'synthetic';
  const caseId = detail.case.case_id;
  const related = detail.evidence.filter(
    (other) =>
      other.evidence_id !== evidence.evidence_id &&
      (other.entity_ids.some((id) => evidence.entity_ids.includes(id)) || other.event_ids.some((id) => evidence.event_ids.includes(id))),
  );
  const reviews = detail.review_items.filter((item) => item.records.some((r) => r.evidence_id === evidence.evidence_id));

  return (
    <Panel
      as="aside"
      title={`${evidence.evidence_id} · artifact`}
      labelledBy="evidence-detail-heading"
      actions={
        <button type="button" onClick={onClose} aria-label="Close evidence detail" className="p-1 text-fg-subtle hover:text-fg">
          <X aria-hidden="true" className="h-4 w-4" />
        </button>
      }
    >
      <div className="border-b border-ink-line p-4">
        <EvidencePreview evidence={evidence} synthetic={synthetic} className="border border-ink-line" />
        <p className="mt-4 break-all font-mono text-sm text-fg">{evidence.original_filename}</p>
        <p className="mt-1 font-mono text-micro uppercase text-fg-subtle">
          {evidence.source} · {formatBytes(evidence.file_size_bytes)}
        </p>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <ProvenanceBadge value={evidence.provenance} withDescription />
          <ReviewBadge value={evidence.review} />
        </div>
      </div>

      <SubHeading>Provenance</SubHeading>
      <ol className="space-y-0 px-4 py-3" aria-label="Artifact to extracted information">
        <li className="relative border-l border-ink-strong pb-3 pl-4">
          <span aria-hidden="true" className="absolute -left-[3px] top-1.5 h-1.5 w-1.5 bg-fg" />
          <p className="font-mono text-micro uppercase text-fg-subtle">Source artifact · read-only original</p>
          <p className="text-sm text-fg">{evidence.source}</p>
        </li>
        <li className="relative border-l border-ink-strong pl-4">
          <span aria-hidden="true" className="absolute -left-[3px] top-1.5 h-1.5 w-1.5 bg-signal-accent" />
          <p className="font-mono text-micro uppercase text-fg-subtle">Extracted information</p>
          <p className="text-sm leading-relaxed text-fg">{evidence.extracted_summary}</p>
        </li>
      </ol>

      <div className="flex items-center justify-between gap-3 border-b border-ink-line px-4 py-3">
        <span className="min-w-0">
          <span className="block font-mono text-micro uppercase text-fg-subtle">SHA-256 · at ingestion</span>
          <span className="block font-mono text-xs text-fg" title={evidence.sha256_hash}>{shortHash(evidence.sha256_hash)}</span>
        </span>
        <CopyHash value={evidence.sha256_hash} />
      </div>

      <SubHeading>Associated entities</SubHeading>
      <LinkList
        empty="No entities linked."
        items={evidence.entity_ids.map((id) => ({
          id,
          label: index.nameOf(id),
          detail: index.entities.get(id)?.type_label,
          href: caseHref(caseId, 'graph', { node: id }),
        }))}
      />
      <SubHeading>Associated events</SubHeading>
      <LinkList
        empty="No events linked."
        items={evidence.event_ids.map((id) => {
          const event = index.events.get(id);
          return {
            id,
            label: event?.title ?? id,
            detail: event ? `${formatEventTime(event.timestamp, event.timestamp_precision)} IST · ${id}` : id,
            href: caseHref(caseId, 'timeline', { event: id }),
          };
        })}
      />
      <SubHeading>Related evidence</SubHeading>
      {related.length === 0 ? (
        <p className="px-4 py-3 text-sm text-fg-subtle">No artifacts share entities or events with this one.</p>
      ) : (
        <ul className="divide-y divide-ink-line">
          {related.map((other) => (
            <li key={other.evidence_id}>
              <button type="button" onClick={() => onSelect(other.evidence_id)} className="flex w-full items-center justify-between gap-3 px-4 py-2.5 text-left hover:bg-white/[0.02]">
                <span className="min-w-0">
                  <span className="block truncate text-sm text-fg">{other.original_filename}</span>
                  <span className="font-mono text-micro text-fg-subtle">{other.evidence_id}</span>
                </span>
                <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <SubHeading>Review</SubHeading>
      {reviews.length === 0 ? (
        <p className="px-4 py-3 text-sm text-fg-subtle">Not referenced by any review item.</p>
      ) : (
        <ul className="divide-y divide-ink-line">
          {reviews.map((item) => (
            <li key={item.id}>
              <Link to={caseHref(caseId, 'contradictions', { item: item.id })} className="flex flex-col gap-1.5 px-4 py-2.5 hover:bg-white/[0.02]">
                <ReviewCategoryBadge category={item.category} />
                <span className="text-sm text-fg">{item.title}</span>
              </Link>
            </li>
          ))}
        </ul>
      )}
      <details className="group border-t border-ink-line">
        <summary className="flex cursor-pointer list-none items-center justify-between bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle hover:text-fg focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-accent">
          Metadata &amp; integrity
          <ChevronDown aria-hidden="true" className="h-3.5 w-3.5 transition-transform group-open:rotate-180" />
        </summary>
      <KeyValueList
        items={[
          ['Evidence type', <><EvidenceTypeLabel evidence={evidence} />{!evidence.source_type && <span className="mt-1 block text-xs text-fg-subtle">{evidence.source_type_label}</span>}</>],
          ['Source', evidence.source],
          ['File size', `${formatBytes(evidence.file_size_bytes)} (${evidence.file_size_bytes.toLocaleString('en-US')} bytes)`],
          ['Ingested', <><span className="block">{formatDateTime(evidence.ingested_at)}</span><span className="font-mono text-micro text-fg-subtle">{formatUtc(evidence.ingested_at)}</span></>],
          ...Object.entries(evidence.metadata).map(([key, value]) => [key, value] as [string, string]),
        ]}
      />

      <p className="border-t border-ink-line px-4 pt-3 font-mono text-micro uppercase text-fg-subtle">Integrity</p>
      <div className="space-y-2 px-4 pb-4 pt-2">
        <p className="font-mono text-micro uppercase text-fg-subtle">SHA-256 · recorded at ingestion</p>
        <p className="break-all font-mono text-xs leading-relaxed text-fg">{evidence.sha256_hash}</p>
        <p className="text-xs leading-relaxed text-fg-subtle">
          Re-hashing the stored copy can detect changes after ingestion. The digest does not establish that the
          artifact was authentic or complete before acquisition.
        </p>
      </div>

      </details>
    </Panel>
  );
};

export const EvidenceRepository: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const index = useCaseIndex(detail);
  const [selectedId, setSelectedId] = useQuerySelection('evidence');
  const [query, setQuery] = useState('');
  const [type, setType] = useState('all');
  const [provenance, setProvenance] = useState('all');
  const [review, setReview] = useState<ReviewStatus | 'all'>('all');
  const [uploadOpen, setUploadOpen] = useState(false);
  const [view, setView] = useState<'gallery' | 'list'>('gallery');
  const synthetic = useWorkspaceRepository().source === 'synthetic';
  const uploadId = useId();

  const typeOptions = useMemo(() => {
    const present = new Set(detail.evidence.map((e) => e.source_type ?? 'UNCLASSIFIED'));
    return [{ value: 'all', label: 'All types' }, ...[...present].sort().map((t) => ({ value: t, label: t === 'UNCLASSIFIED' ? 'Unclassified' : t }))];
  }, [detail.evidence]);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return detail.evidence.filter(
      (e) =>
        (type === 'all' || (e.source_type ?? 'UNCLASSIFIED') === type) &&
        (provenance === 'all' || e.provenance === provenance) &&
        (review === 'all' || e.review === review) &&
        (!q || [e.evidence_id, e.original_filename, e.source, e.extracted_summary].some((f) => f.toLowerCase().includes(q))),
    );
  }, [detail.evidence, query, type, provenance, review]);

  const selected = selectedId ? index.evidence.get(selectedId) : undefined;
  const reviewCount = (value: ReviewStatus | 'all'): number =>
    value === 'all' ? detail.evidence.length : detail.evidence.filter((e) => e.review === value).length;

  const gallery = (
    <ul aria-label="Evidence gallery" className="grid grid-cols-1 gap-3 p-3 sm:grid-cols-2 sm:p-4 2xl:grid-cols-3">
      {rows.map((e) => {
        const isSelected = e.evidence_id === selectedId;
        return (
          <li key={e.evidence_id}>
            <button
              type="button"
              aria-pressed={isSelected}
              onClick={() => setSelectedId(isSelected ? null : e.evidence_id)}
              className={`group block w-full border text-left transition-[border-color,transform,box-shadow] duration-base ease-out focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-accent motion-safe:hover:-translate-y-0.5 ${
                isSelected ? 'border-crimson/70 bg-ink-850 shadow-[0_18px_30px_-18px_rgba(227,38,54,.45)]' : 'border-ink-line bg-ink-950 hover:border-ink-strong hover:shadow-[0_18px_30px_-18px_rgba(0,0,0,.9)]'
              }`}
            >
              <EvidencePreview evidence={e} synthetic={synthetic} className="border-b border-ink-line" />
              <span className="block px-3 py-2.5">
                <span className="flex items-center justify-between gap-2">
                  <span className={`font-mono text-micro ${e.review === 'flagged' ? 'text-crimson' : 'text-fg-subtle'}`}>{e.evidence_id}</span>
                  <ReviewBadge value={e.review} />
                </span>
                <span className={`mt-1 block truncate text-sm ${isSelected ? 'text-fg-bright' : 'text-fg'}`}>{e.original_filename}</span>
                <span className="mt-0.5 block truncate font-mono text-micro uppercase text-fg-subtle">
                  {e.source_type ?? 'Unclassified'} · {formatDateTime(e.ingested_at)}
                </span>
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );

  return (
    <div className="space-y-4 p-4 sm:p-6 lg:p-8">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="font-mono text-micro uppercase text-fg-subtle">
          {detail.evidence.length} artifacts · originals read-only
        </p>
        <WsButton icon={Plus} aria-expanded={uploadOpen} aria-controls={uploadId} onClick={() => setUploadOpen((value) => !value)}>
          {uploadOpen ? 'Hide staging' : 'Add evidence'}
        </WsButton>
      </div>
      {uploadOpen && <EvidenceUploadPanel id={uploadId} caseId={detail.case.case_id} />}

      <div className={`grid grid-cols-1 gap-4 ${selected ? 'xl:grid-cols-12' : ''}`}>
        <Panel
          title="Evidence repository"
          meta={`${rows.length} of ${detail.evidence.length}`}
          className={selected ? 'xl:col-span-7' : ''}
          actions={
            <div role="group" aria-label="Evidence view" className="flex border border-ink-line">
              {(
                [
                  ['gallery', 'Gallery view', LayoutGrid],
                  ['list', 'List view', List],
                ] as const
              ).map(([value, label, Icon]) => (
                <button
                  key={value}
                  type="button"
                  aria-label={label}
                  aria-pressed={view === value}
                  onClick={() => setView(value)}
                  className={`p-1.5 transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent ${view === value ? 'bg-ink-700 text-fg' : 'text-fg-subtle hover:text-fg'}`}
                >
                  <Icon aria-hidden="true" className="h-3.5 w-3.5" />
                </button>
              ))}
            </div>
          }
        >
          <div className="flex flex-col gap-3 border-b border-ink-line p-3 sm:p-4">
            <div className="grid gap-2 md:grid-cols-[1fr_auto_auto]">
              <SearchField value={query} onChange={setQuery} label="Search evidence" placeholder="ID, file name, source or extracted text" />
              <SelectField label="Type" value={type} onChange={setType} options={typeOptions} />
              <SelectField
                label="Provenance"
                value={provenance}
                onChange={setProvenance}
                options={[
                  { value: 'all', label: 'All' },
                  { value: 'observed', label: 'Observed' },
                  { value: 'supplied', label: 'Supplied' },
                ]}
              />
            </div>
            <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filter by review status">
              {REVIEW_FILTERS.map((filter) => (
                <FilterChip key={filter.value} active={review === filter.value} onClick={() => setReview(filter.value)} count={reviewCount(filter.value)}>
                  {filter.label}
                </FilterChip>
              ))}
            </div>
          </div>

          {rows.length === 0 ? (
            <EmptyState icon={FileSearch} title="No matching evidence">
              Adjust the search or filters.
            </EmptyState>
          ) : view === 'gallery' ? (
            gallery
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead>
                  <tr className="border-b border-ink-line font-mono text-micro uppercase text-fg-subtle">
                    <th scope="col" className="px-4 py-2.5 font-normal">Artifact</th>
                    <th scope="col" className="hidden px-4 py-2.5 font-normal sm:table-cell">Type</th>
                    <th scope="col" className="hidden px-4 py-2.5 font-normal 2xl:table-cell">Ingested</th>
                    <th scope="col" className="hidden px-4 py-2.5 font-normal md:table-cell">SHA-256</th>
                    <th scope="col" className="px-4 py-2.5 font-normal">Review</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-ink-line">
                  {rows.map((e) => {
                    const isSelected = e.evidence_id === selectedId;
                    return (
                      <tr key={e.evidence_id} className={`transition-colors ${isSelected ? 'bg-signal-accent/[0.07]' : 'hover:bg-white/[0.02]'}`}>
                        <td className="px-4 py-3">
                          <button
                            type="button"
                            aria-pressed={isSelected}
                            onClick={() => setSelectedId(isSelected ? null : e.evidence_id)}
                            className="block min-w-0 max-w-[20rem] break-all text-left focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent"
                          >
                            <span className="font-mono text-micro text-fg-subtle">{e.evidence_id} · {e.source}</span>
                            <span className={`block truncate text-sm ${isSelected ? 'text-signal-accent' : 'text-fg'}`}>{e.original_filename}</span>
                          </button>
                        </td>
                        <td className="hidden px-4 py-3 sm:table-cell"><EvidenceTypeLabel evidence={e} /></td>
                        <td className="hidden px-4 py-3 font-mono text-xs text-fg-muted 2xl:table-cell">{formatDateTime(e.ingested_at)}</td>
                        <td className="hidden px-4 py-3 font-mono text-xs text-fg-muted md:table-cell" title={e.sha256_hash}>{shortHash(e.sha256_hash)}</td>
                        <td className="px-4 py-3"><ReviewBadge value={e.review} /></td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </Panel>

        {selected && (
          <div className="ws-inspector-enter xl:col-span-5">
            <EvidenceDetailPanel evidence={selected} detail={detail} index={index} onSelect={setSelectedId} onClose={() => setSelectedId(null)} />
          </div>
        )}
      </div>
    </div>
  );
};
