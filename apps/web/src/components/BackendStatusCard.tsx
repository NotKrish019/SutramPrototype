import React from 'react';
import { RefreshCw } from 'lucide-react';
import type { ConnectionStatus, UseBackendHealthResult } from '../hooks/useBackendHealth';

export interface BackendStatusCardProps {
  health: UseBackendHealthResult;
  /** Health URL shown while checking (already resolved; never throws). */
  healthUrl: string;
  pollIntervalMs: number;
}

const STATUS_STYLES: Record<ConnectionStatus, { label: string; dot: string; text: string }> = {
  checking: { label: 'Checking', dot: 'bg-signal-amber animate-pulse motion-reduce:animate-none', text: 'text-signal-amber' },
  connected: { label: 'Connected', dot: 'bg-signal-ok', text: 'text-signal-ok' },
  offline: { label: 'Offline', dot: 'bg-signal-flag', text: 'text-signal-flag' },
};

function formatTime(date: Date): string {
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

/** Presentational card for backend connectivity. State comes from `useBackendHealth`. */
export const BackendStatusCard: React.FC<BackendStatusCardProps> = ({
  health,
  healthUrl,
  pollIntervalMs,
}) => {
  const { status, data, error, lastSuccessAt, isChecking, retry } = health;
  const style = STATUS_STYLES[status];

  return (
    <section
      aria-labelledby="backend-status-heading"
      className="border border-ink-line bg-ink-900"
    >
      <div className="flex min-h-[2.75rem] flex-wrap items-center justify-between gap-3 border-b border-ink-line px-4 py-2">
        <h2 id="backend-status-heading" className="font-mono text-micro uppercase text-fg-muted">
          Backend Gateway Telemetry
        </h2>
        <button
          type="button"
          onClick={retry}
          disabled={isChecking}
          className="inline-flex h-8 items-center gap-2 border border-ink-strong px-2.5 font-mono text-micro uppercase tracking-[0.04em] text-fg transition-colors hover:border-fg-muted hover:bg-white/[0.03] focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-amber disabled:cursor-not-allowed disabled:opacity-45"
        >
          <RefreshCw aria-hidden="true" className={`h-3.5 w-3.5 ${isChecking ? 'animate-spin motion-reduce:animate-none' : ''}`} />
          Retry Now
        </button>
      </div>

      <div className="flex items-start gap-4 px-4 py-4">
        <div aria-hidden="true" className={`mt-1.5 h-2.5 w-2.5 shrink-0 ${style.dot}`} />
        <div className="min-w-0 space-y-1.5 text-sm">
          <p role="status" aria-live="polite" className={`font-display text-xl font-semibold uppercase leading-none ${style.text}`}>
            {style.label}
          </p>

          {status === 'checking' && (
            <p className="text-fg-muted">Pinging backend at {healthUrl}...</p>
          )}

          {status === 'connected' && data && (
            <p className="font-mono text-xs text-fg">
              {data.project} (Status: {data.status})
            </p>
          )}

          {status === 'offline' && (
            <>
              <p className="font-mono text-xs text-fg-muted">
                Run <code className="text-signal-amber">uv run uvicorn app.main:app</code> in{' '}
                <code className="text-fg">backend/</code>
              </p>
              {error && (
                <p role="alert" className="break-all font-mono text-xs text-signal-flag">
                  {error.kind !== 'unknown' && (
                    <span className="uppercase text-signal-flag">[{error.kind}] </span>
                  )}
                  {error.message}
                </p>
              )}
            </>
          )}

          <p className="font-mono text-micro uppercase text-fg-subtle">
            Last successful check:{' '}
            {lastSuccessAt ? (
              <time dateTime={lastSuccessAt.toISOString()} className="text-fg-muted">
                {formatTime(lastSuccessAt)}
              </time>
            ) : (
              <span className="text-fg-muted">Never</span>
            )}
            {' · '}Auto-checks every {Math.round(pollIntervalMs / 1000)}s
          </p>
        </div>
      </div>
    </section>
  );
};
