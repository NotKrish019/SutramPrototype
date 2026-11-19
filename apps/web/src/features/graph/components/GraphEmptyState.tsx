import React from 'react';
import { Database, Filter, Loader2, ServerOff } from 'lucide-react';

export interface GraphEmptyStateProps {
  isLoading: boolean;
  isDegraded?: boolean;
  hasFiltersApplied?: boolean;
  onResetFilters?: () => void;
  onRetry?: () => void;
}

export const GraphEmptyState: React.FC<GraphEmptyStateProps> = ({
  isLoading,
  isDegraded,
  hasFiltersApplied,
  onResetFilters,
  onRetry,
}) => {
  if (isLoading) {
    return (
      <div className="flex h-96 flex-col items-center justify-center p-8 text-center">
        <Loader2 className="h-8 w-8 animate-spin text-signal-amber" />
        <p className="mt-4 font-mono text-sm uppercase tracking-wider text-fg">
          Querying Forensic Knowledge Graph…
        </p>
        <p className="mt-1 font-mono text-xs text-fg-subtle">
          Fetching case topology, entity candidates, and relationship assertions.
        </p>
      </div>
    );
  }

  if (hasFiltersApplied) {
    return (
      <div className="flex h-96 flex-col items-center justify-center p-8 text-center">
        <Filter className="h-8 w-8 text-fg-subtle" />
        <p className="mt-4 font-mono text-sm uppercase tracking-wider text-fg">
          No Elements Match Active Filters
        </p>
        <p className="mt-1 font-mono text-xs text-fg-subtle">
          Adjust category or relationship filters to display hidden graph objects.
        </p>
        {onResetFilters && (
          <button
            type="button"
            onClick={onResetFilters}
            className="mt-4 rounded border border-ink-line bg-ink-900 px-3 py-1.5 font-mono text-xs text-fg hover:bg-white/[0.04]"
          >
            Reset Filters
          </button>
        )}
      </div>
    );
  }

  if (isDegraded) {
    return (
      <div className="flex h-96 flex-col items-center justify-center p-8 text-center">
        <ServerOff className="h-8 w-8 text-signal-amber" />
        <p className="mt-4 font-mono text-sm uppercase tracking-wider text-fg">
          Graph Subsystem Offline or Unconfigured
        </p>
        <p className="mt-1 max-w-md font-mono text-xs text-fg-subtle">
          The Rust backend is operating in filesystem fallback mode without Neo4j projection.
          Relational ground truth remains intact.
        </p>
        {onRetry && (
          <button
            type="button"
            onClick={onRetry}
            className="mt-4 rounded bg-signal-amber px-3 py-1.5 font-mono text-xs font-semibold text-ink-950 hover:opacity-90"
          >
            Retry Connection
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="flex h-96 flex-col items-center justify-center p-8 text-center">
      <Database className="h-8 w-8 text-fg-subtle" />
      <p className="mt-4 font-mono text-sm uppercase tracking-wider text-fg">
        No Graph Data Ingested for Case
      </p>
      <p className="mt-1 font-mono text-xs text-fg-subtle">
        Ingest evidence files or execute extraction runs to project entities and events.
      </p>
    </div>
  );
};
