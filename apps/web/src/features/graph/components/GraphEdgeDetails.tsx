import React from 'react';
import { ArrowRight, Link as LinkIcon, Shield } from 'lucide-react';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import type { CytoscapeEdgeData } from '../types/graph';

export interface GraphEdgeDetailsProps {
  caseId: string;
  edge: CytoscapeEdgeData;
  sourceLabel?: string;
  targetLabel?: string;
}

export const GraphEdgeDetails: React.FC<GraphEdgeDetailsProps> = ({
  caseId,
  edge,
  sourceLabel,
  targetLabel,
}) => {
  const p = edge.rawProperties || {};
  const evidenceBasis: string[] = Array.isArray(p.evidence_basis)
    ? (p.evidence_basis as string[])
    : [];

  return (
    <div className="space-y-4 p-4">
      {/* Header */}
      <div className="border-b border-ink-line pb-3">
        <div className="flex items-center gap-2">
          <span
            className={`rounded px-1.5 py-0.5 font-mono text-micro uppercase ${
              edge.isContradiction
                ? 'bg-signal-flag/20 text-signal-flag'
                : 'bg-ink-850 text-fg-subtle'
            }`}
          >
            {edge.relationType}
          </span>
          <span
            className={`rounded px-1.5 py-0.5 font-mono text-micro uppercase ${
              edge.isObserved
                ? 'bg-white/10 text-fg'
                : 'bg-signal-amber/15 text-signal-amber'
            }`}
          >
            {edge.isObserved ? 'Observed Fact' : 'Inferred Link'}
          </span>
        </div>

        <h3 className="mt-2 font-display text-base font-bold text-fg">
          {sourceLabel || edge.source}{' '}
          <span className="text-fg-subtle font-mono text-xs">—[{edge.label}]→</span>{' '}
          {targetLabel || edge.target}
        </h3>
      </div>

      {/* Attributes */}
      <div className="space-y-2 font-mono text-xs">
        <div className="flex justify-between border-b border-ink-line/50 py-1">
          <span className="text-fg-subtle">Relationship ID</span>
          <span className="truncate text-fg" title={edge.id}>
            {edge.id}
          </span>
        </div>

        {edge.confidence !== null && edge.confidence !== undefined && (
          <div className="flex justify-between border-b border-ink-line/50 py-1">
            <span className="text-fg-subtle">Weight / Confidence</span>
            <span className="text-signal-amber font-semibold">
              {(edge.confidence * 100).toFixed(0)}%
            </span>
          </div>
        )}

        {typeof p.assertion_id === 'string' && (
          <div className="flex justify-between border-b border-ink-line/50 py-1">
            <span className="text-fg-subtle">Assertion ID</span>
            <span className="text-fg">{p.assertion_id}</span>
          </div>
        )}
      </div>

      {/* Supporting Evidence */}
      <div className="space-y-2 border-t border-ink-line pt-3">
        <h4 className="flex items-center gap-1.5 font-mono text-micro uppercase tracking-wider text-fg-subtle">
          <LinkIcon className="h-3 w-3 text-signal-cyan" />
          <span>Supporting Evidence Artifacts</span>
        </h4>

        {evidenceBasis.length === 0 ? (
          <p className="font-mono text-micro text-fg-subtle">
            No specific evidence artifact linked directly to this relationship.
          </p>
        ) : (
          <ul className="space-y-1.5">
            {evidenceBasis.map((evId) => (
              <li key={evId}>
                <Link
                  to={caseHref(caseId, 'evidence', { evidence: evId })}
                  className="flex items-center justify-between rounded border border-ink-line bg-ink-900 px-2.5 py-1.5 text-xs text-fg transition-colors hover:bg-white/[0.04]"
                >
                  <span className="font-mono">{evId}</span>
                  <ArrowRight className="h-3 w-3 text-fg-subtle" />
                </Link>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Forensic Disclaimer */}
      <div className="rounded border border-ink-line bg-ink-950 p-2.5 text-micro leading-relaxed text-fg-subtle">
        <div className="flex items-center gap-1.5 font-semibold text-fg">
          <Shield className="h-3.5 w-3.5 text-signal-amber" />
          <span>Analytical Disclaimer</span>
        </div>
        <p className="mt-1">
          {edge.isObserved
            ? 'Observed relationships are directly extracted from primary telecommunication CDR or electronic chat logs.'
            : 'Inferred associations are generated through temporal and spatial correlation algorithms. They require manual investigator verification.'}
        </p>
      </div>
    </div>
  );
};
