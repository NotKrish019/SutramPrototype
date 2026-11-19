import React, { useMemo, useState } from 'react';
import { ProvenanceBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { useCaseIndex, useQuerySelection } from '../caseIndex';
import { FilterChip, Panel } from '../components/ui';
import type { CaseDetail, ProvenanceClass } from '../types';

type ProvenanceFilter = ProvenanceClass | 'all';

export const CorrelationWorkspace: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const index = useCaseIndex(detail);
  const caseId = detail.case.case_id;
  const [selectedRel, setSelectedRel] = useQuerySelection('rel');
  const [filter, setFilter] = useState<ProvenanceFilter>('all');
  const [hoverEntity, setHoverEntity] = useState<string | null>(null);

  const entityRelationships = useMemo(
    () => detail.relationships.filter((r) => index.entities.has(r.source_id) && index.entities.has(r.target_id)),
    [detail.relationships, index.entities],
  );
  const visible = entityRelationships.filter((r) => filter === 'all' || r.provenance === filter);
  const active = entityRelationships.find((r) => r.relationship_id === selectedRel) ?? null;

  const isHighlighted = (evidenceId: string, entityId: string): boolean => {
    if (active) return active.evidence_basis.includes(evidenceId) && (entityId === active.source_id || entityId === active.target_id);
    if (hoverEntity) return entityId === hoverEntity;
    return false;
  };

  const counts: Record<ProvenanceFilter, number> = {
    all: entityRelationships.length,
    observed: entityRelationships.filter((r) => r.provenance === 'observed').length,
    supplied: entityRelationships.filter((r) => r.provenance === 'supplied').length,
    inferred: entityRelationships.filter((r) => r.provenance === 'inferred').length,
  };

  return (
    <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
      <Panel title="Evidence × entity matrix" meta="which artifact supports which entity" className="xl:col-span-7">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[38rem] border-collapse text-left">
            <caption className="sr-only">
              Rows are evidence artifacts, columns are entities. A filled cell means the entity was extracted from that artifact.
            </caption>
            <thead>
              <tr>
                <th scope="col" className="border-b border-ink-line px-3 py-2 font-mono text-micro font-normal uppercase text-fg-subtle">Artifact</th>
                {detail.entities.map((entity) => (
                  <th
                    key={entity.entity_id}
                    scope="col"
                    onMouseEnter={() => setHoverEntity(entity.entity_id)}
                    onMouseLeave={() => setHoverEntity(null)}
                    className={`border-b border-l border-ink-line px-2 py-2 align-bottom font-normal ${hoverEntity === entity.entity_id ? 'bg-white/[0.03]' : ''}`}
                  >
                    <span className="block text-xs text-fg">{entity.canonical_name}</span>
                    <span className="block font-mono text-[0.625rem] uppercase text-fg-subtle">{entity.entity_type ?? '—'}</span>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {detail.evidence.map((evidence) => (
                <tr key={evidence.evidence_id}>
                  <th scope="row" className="border-b border-ink-line px-3 py-2.5 font-normal">
                    <Link to={caseHref(caseId, 'evidence', { evidence: evidence.evidence_id })} className="block hover:text-signal-accent">
                      <span className="font-mono text-xs text-fg">{evidence.evidence_id}</span>
                      <span className="block max-w-[11rem] truncate font-mono text-[0.625rem] text-fg-subtle">{evidence.source}</span>
                    </Link>
                  </th>
                  {detail.entities.map((entity) => {
                    const linked = evidence.entity_ids.includes(entity.entity_id);
                    const lit = isHighlighted(evidence.evidence_id, entity.entity_id);
                    return (
                      <td
                        key={entity.entity_id}
                        className={`border-b border-l border-ink-line text-center transition-colors ${lit ? 'bg-signal-accent/15' : ''}`}
                      >
                        {linked ? (
                          <span
                            role="img"
                            aria-label={`${entity.canonical_name} extracted from ${evidence.evidence_id}`}
                            className={`mx-auto block h-2.5 w-2.5 ${lit ? 'bg-signal-accent' : 'bg-fg'}`}
                          />
                        ) : (
                          <span aria-hidden="true" className="text-fg-subtle/40">·</span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="border-t border-ink-line px-4 py-2.5 text-xs text-fg-subtle">
          {active
            ? `Highlighting the artifacts that support “${index.nameOf(active.source_id)} — ${active.label} → ${index.nameOf(active.target_id)}”.`
            : 'Select a relationship to see the artifacts it rests on. Hover an entity column to trace it.'}
        </p>
      </Panel>

      <Panel title="Relationships & evidence basis" meta={`${visible.length} shown`} className="xl:col-span-5">
        <div className="flex flex-wrap gap-1.5 border-b border-ink-line p-3" role="group" aria-label="Filter relationships by provenance">
          {(['all', 'observed', 'supplied', 'inferred'] as ProvenanceFilter[]).map((value) => (
            <FilterChip key={value} active={filter === value} onClick={() => setFilter(value)} count={counts[value]}>
              {value === 'all' ? 'All' : value}
            </FilterChip>
          ))}
        </div>
        <ul className="divide-y divide-ink-line">
          {visible.map((rel) => {
            const selected = rel.relationship_id === selectedRel;
            return (
              <li key={rel.relationship_id}>
                <button
                  type="button"
                  aria-pressed={selected}
                  onClick={() => setSelectedRel(selected ? null : rel.relationship_id)}
                  className={`w-full px-4 py-3 text-left transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-accent ${
                    selected ? 'bg-signal-accent/[0.07]' : 'hover:bg-white/[0.02]'
                  }`}
                >
                  <span className="flex flex-wrap items-center justify-between gap-2">
                    <span className="text-sm text-fg">
                      {index.nameOf(rel.source_id)} <span className="text-fg-muted">— {rel.label} →</span> {index.nameOf(rel.target_id)}
                    </span>
                    <ProvenanceBadge value={rel.provenance} />
                  </span>
                  <span className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1 font-mono text-micro uppercase text-fg-subtle">
                    <span>{rel.relation_type}</span>
                    <span>is_observed: {String(rel.is_observed)}</span>
                    <span>weight: {rel.weight === null ? 'not provided' : rel.weight.toFixed(2)}</span>
                    <span>basis: {rel.evidence_basis.length > 0 ? rel.evidence_basis.join(', ') : 'record outside case evidence'}</span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
        <p className="border-t border-ink-line px-4 py-2.5 text-xs text-fg-subtle">
          Inferred links are proposals for review, not findings. A correlation engine is planned; these links are part of the synthetic case.
        </p>
      </Panel>
    </div>
  );
};
