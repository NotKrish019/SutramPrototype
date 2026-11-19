import React from 'react';
import { AlertTriangle, CheckCircle, Database, RefreshCw, ShieldCheck } from 'lucide-react';
import type { GraphSummaryDto } from '../types/graph';

export const GraphSyncStatus: React.FC<{
  summary: GraphSummaryDto | null;
  isLoading: boolean;
  onRefresh?: () => void;
}> = ({ summary, isLoading, onRefresh }) => {
  if (!summary) {
    return (
      <div className="flex items-center gap-2 rounded border border-ink-line bg-ink-900 px-3 py-1.5 font-mono text-micro text-fg-muted">
        <Database className="h-3.5 w-3.5 animate-pulse text-fg-subtle" />
        <span>PROJECTION: CHECKING STATUS…</span>
      </div>
    );
  }

  const rawStatus = summary.graph_synchronization_status || 'UNKNOWN';
  let badgeColor = 'text-fg-subtle border-ink-line bg-ink-900';
  let icon = <Database className="h-3.5 w-3.5" />;

  if (rawStatus === 'SYNCHRONIZED') {
    badgeColor = 'text-signal-green border-signal-green/30 bg-signal-green/10';
    icon = <CheckCircle className="h-3.5 w-3.5 text-signal-green" />;
  } else if (rawStatus.startsWith('SYNCING')) {
    badgeColor = 'text-signal-amber border-signal-amber/30 bg-signal-amber/10';
    icon = <RefreshCw className="h-3.5 w-3.5 animate-spin text-signal-amber" />;
  } else if (rawStatus.startsWith('DEGRADED') || rawStatus === 'FAILED') {
    badgeColor = 'text-signal-flag border-signal-flag/30 bg-signal-flag/10';
    icon = <AlertTriangle className="h-3.5 w-3.5 text-signal-flag" />;
  } else if (rawStatus === 'GRAPH_DISABLED') {
    badgeColor = 'text-fg-subtle border-ink-line bg-ink-850';
    icon = <Database className="h-3.5 w-3.5 text-fg-subtle" />;
  }

  const isPrivacyActive = summary.identifier_privacy === 'hmac_sha256_active';

  return (
    <div className="flex flex-wrap items-center gap-2">
      <div
        className={`flex items-center gap-1.5 rounded border px-2.5 py-1 font-mono text-micro uppercase tracking-wide ${badgeColor}`}
        title={`Knowledge graph outbox projection status: ${rawStatus}`}
      >
        {icon}
        <span>GRAPH: {rawStatus}</span>
      </div>

      <div
        className={`flex items-center gap-1.5 rounded border px-2.5 py-1 font-mono text-micro uppercase tracking-wide ${
          isPrivacyActive
            ? 'border-signal-cyan/30 bg-signal-cyan/10 text-signal-cyan'
            : 'border-ink-line bg-ink-850 text-fg-subtle'
        }`}
        title={
          isPrivacyActive
            ? 'Keyed HMAC-SHA-256 privacy enabled for identifiers'
            : 'Identifier privacy unkeyed fallback'
        }
      >
        <ShieldCheck className="h-3.5 w-3.5" />
        <span>{isPrivacyActive ? 'PRIVACY: HMAC-SHA256' : 'PRIVACY: STANDARD'}</span>
      </div>

      {onRefresh && (
        <button
          type="button"
          onClick={onRefresh}
          disabled={isLoading}
          aria-label="Refresh graph synchronization"
          className="flex h-7 w-7 items-center justify-center rounded border border-ink-line bg-ink-900 text-fg-subtle transition-colors hover:bg-white/[0.04] hover:text-fg disabled:opacity-50"
        >
          <RefreshCw className={`h-3 w-3 ${isLoading ? 'animate-spin' : ''}`} />
        </button>
      )}
    </div>
  );
};
