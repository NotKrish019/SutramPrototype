import React from 'react';
import { ArrowRight, Compass, Network, ShieldAlert } from 'lucide-react';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import type { CytoscapeElement, CytoscapeNodeData } from '../types/graph';
import { CATEGORY_LABELS } from '../utils/graphFilters';

export interface GraphNodeDetailsProps {
  caseId: string;
  node: CytoscapeNodeData;
  availableNodes: CytoscapeElement[];
  isExpanding: boolean;
  isTracingPath: boolean;
  pathTargetId: string;
  onSelectPathTarget: (id: string) => void;
  onExpandNeighbors: (nodeId: string) => void;
  onTracePath: (fromId: string, toId: string) => void;
  onClearPath: () => void;
  pathFound?: boolean | null;
}

export const GraphNodeDetails: React.FC<GraphNodeDetailsProps> = ({
  caseId,
  node,
  availableNodes,
  isExpanding,
  isTracingPath,
  pathTargetId,
  onSelectPathTarget,
  onExpandNeighbors,
  onTracePath,
  onClearPath,
  pathFound,
}) => {
  const p = node.rawProperties || {};
  const isCandidate =
    node.category === 'EntityCandidate' || node.category === 'EventCandidate';

  const otherNodes = availableNodes
    .filter((el) => el.group === 'nodes' && el.data.id !== node.id)
    .map((el) => el.data as CytoscapeNodeData);

  return (
    <div className="space-y-4 p-4">
      {/* Header */}
      <div className="border-b border-ink-line pb-3">
        <div className="flex items-center gap-2">
          <span className="rounded bg-ink-850 px-1.5 py-0.5 font-mono text-micro uppercase text-fg-subtle">
            {CATEGORY_LABELS[node.category] || node.category}
          </span>
          {node.isRedacted && (
            <span className="flex items-center gap-1 rounded bg-signal-cyan/15 px-1.5 py-0.5 font-mono text-micro uppercase text-signal-cyan">
              <ShieldAlert className="h-3 w-3" /> HMAC Protected
            </span>
          )}
        </div>
        <h3 className="mt-1.5 font-display text-lg font-bold text-fg">{node.label}</h3>
        <p className="font-mono text-micro uppercase text-fg-subtle">{node.sublabel}</p>
      </div>

      {/* Attributes List */}
      <div className="space-y-2 font-mono text-xs">
        <div className="flex justify-between border-b border-ink-line/50 py-1">
          <span className="text-fg-subtle">Node ID</span>
          <span className="text-fg" title={node.id}>
            {node.id.length > 22 ? `${node.id.slice(0, 10)}…${node.id.slice(-8)}` : node.id}
          </span>
        </div>

        {node.confidence !== null && node.confidence !== undefined && (
          <div className="flex justify-between border-b border-ink-line/50 py-1">
            <span className="text-fg-subtle">Confidence</span>
            <span className="text-signal-amber font-semibold">
              {(node.confidence * 100).toFixed(0)}%
            </span>
          </div>
        )}

        {typeof p.sha256_hash === 'string' && (
          <div className="flex justify-between border-b border-ink-line/50 py-1">
            <span className="text-fg-subtle">SHA-256</span>
            <span className="truncate text-fg" title={p.sha256_hash}>
              {p.sha256_hash.slice(0, 12)}…
            </span>
          </div>
        )}

        {typeof p.timestamp === 'string' && (
          <div className="flex justify-between border-b border-ink-line/50 py-1">
            <span className="text-fg-subtle">Timestamp</span>
            <span className="text-fg">{p.timestamp}</span>
          </div>
        )}

        {typeof p.ordering === 'string' && (
          <div className="flex justify-between border-b border-ink-line/50 py-1">
            <span className="text-fg-subtle">Ordering</span>
            <span className="text-fg">{p.ordering}</span>
          </div>
        )}
      </div>

      {/* Forensic Disclaimers */}
      {node.isRedacted && (
        <div className="rounded border border-signal-cyan/20 bg-signal-cyan/5 p-2.5 text-xs text-signal-cyan">
          <p className="font-semibold">Privacy Safeguard Active</p>
          <p className="mt-1 text-micro text-fg-muted">
            The technical identifier (phone/IMEI/IMSI) has been transformed via case-scoped
            HMAC-SHA-256. Plaintext ground truth is isolated in PostgreSQL.
          </p>
        </div>
      )}

      {/* Actions: Neighborhood Expansion & Path Tracing */}
      <div className="space-y-3 border-t border-ink-line pt-3">
        <h4 className="font-mono text-micro uppercase tracking-wider text-fg-subtle">
          Forensic Graph Actions
        </h4>

        {/* Expand Node */}
        {isCandidate && (
          <button
            type="button"
            onClick={() => onExpandNeighbors(node.id)}
            disabled={isExpanding}
            className="flex w-full items-center justify-center gap-2 rounded border border-ink-line bg-ink-900 py-2 font-mono text-xs text-fg transition-colors hover:bg-white/[0.04] disabled:opacity-50"
          >
            <Network className={`h-3.5 w-3.5 ${isExpanding ? 'animate-spin' : ''}`} />
            <span>{isExpanding ? 'Expanding Neighbors…' : 'Expand 1-Hop Neighbors'}</span>
          </button>
        )}

        {/* Trace Path */}
        <div className="space-y-2 rounded border border-ink-line bg-ink-950 p-2.5">
          <label
            htmlFor="path-target-select"
            className="flex items-center gap-1.5 font-mono text-micro uppercase text-fg-subtle"
          >
            <Compass className="h-3 w-3 text-signal-amber" />
            <span>Trace Path To Target Entity</span>
          </label>
          <select
            id="path-target-select"
            value={pathTargetId}
            onChange={(e) => onSelectPathTarget(e.target.value)}
            className="w-full rounded border border-ink-line bg-ink-900 px-2 py-1.5 font-mono text-xs text-fg focus:border-signal-amber focus:outline-none"
          >
            <option value="">Select target node…</option>
            {otherNodes.map((other) => (
              <option key={other.id} value={other.id}>
                {other.label} ({CATEGORY_LABELS[other.category] || other.category})
              </option>
            ))}
          </select>

          {pathTargetId && (
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => onTracePath(node.id, pathTargetId)}
                disabled={isTracingPath}
                className="flex flex-1 items-center justify-center gap-1.5 rounded bg-signal-amber py-1.5 font-mono text-xs font-semibold text-ink-950 transition-opacity hover:opacity-90 disabled:opacity-50"
              >
                <span>{isTracingPath ? 'Tracing…' : 'Trace Path'}</span>
              </button>
              {pathFound !== null && pathFound !== undefined && (
                <button
                  type="button"
                  onClick={onClearPath}
                  className="rounded border border-ink-line px-2 py-1.5 font-mono text-xs text-fg-subtle hover:text-fg"
                >
                  Clear
                </button>
              )}
            </div>
          )}

          {pathFound === true && (
            <p className="text-micro text-signal-amber">
              Evidentiary path located and highlighted in amber.
            </p>
          )}
          {pathFound === false && (
            <p className="text-micro text-signal-flag">
              No evidentiary path found within 5 hops.
            </p>
          )}
        </div>
      </div>

      {/* Cross-Section Navigation Links */}
      <div className="border-t border-ink-line pt-3">
        {node.category === 'EventCandidate' && (
          <Link
            to={caseHref(caseId, 'timeline', { event: node.id })}
            className="flex items-center justify-between text-xs text-signal-amber hover:underline"
          >
            <span>View in Chronological Timeline</span>
            <ArrowRight className="h-3 w-3" />
          </Link>
        )}
        {node.category === 'Evidence' && (
          <Link
            to={caseHref(caseId, 'evidence', { evidence: node.id })}
            className="flex items-center justify-between text-xs text-signal-cyan hover:underline"
          >
            <span>Inspect Evidence Artifact & Hash</span>
            <ArrowRight className="h-3 w-3" />
          </Link>
        )}
        {node.category === 'ContradictionLead' && (
          <Link
            to={caseHref(caseId, 'contradictions', { item: node.id })}
            className="flex items-center justify-between text-xs text-signal-flag hover:underline"
          >
            <span>Open Contradiction Review Workspace</span>
            <ArrowRight className="h-3 w-3" />
          </Link>
        )}
      </div>
    </div>
  );
};
