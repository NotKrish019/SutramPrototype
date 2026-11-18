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
  checking: { label: 'Checking', dot: 'bg-amber-500 animate-pulse', text: 'text-amber-400' },
  connected: { label: 'Connected', dot: 'bg-emerald-500', text: 'text-emerald-400' },
  offline: { label: 'Offline', dot: 'bg-rose-500', text: 'text-rose-400' },
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
      className="bg-slate-900/50 border border-slate-800 rounded-xl p-6"
    >
      <div className="flex flex-wrap items-center justify-between gap-3 mb-4">
        <h3
          id="backend-status-heading"
          className="text-sm font-semibold text-slate-200 uppercase tracking-wider"
        >
          Backend Gateway Telemetry
        </h3>
        <button
          type="button"
          onClick={retry}
          disabled={isChecking}
          className="inline-flex items-center gap-2 rounded-md border border-slate-700 bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-200 hover:bg-slate-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-500 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <RefreshCw aria-hidden="true" className={`w-3.5 h-3.5 ${isChecking ? 'animate-spin' : ''}`} />
          Retry Now
        </button>
      </div>

      <div className="flex items-start space-x-4">
        <div aria-hidden="true" className={`mt-1.5 w-3 h-3 shrink-0 rounded-full ${style.dot}`} />
        <div className="text-sm space-y-1 min-w-0">
          <p role="status" aria-live="polite" className={`font-semibold ${style.text}`}>
            {style.label}
          </p>

          {status === 'checking' && (
            <p className="text-slate-400">Pinging backend at {healthUrl}...</p>
          )}

          {status === 'connected' && data && (
            <p className="text-emerald-400 font-mono">
              {data.project} (Status: {data.status})
            </p>
          )}

          {status === 'offline' && (
            <>
              <p className="text-slate-400 font-mono">
                Run <code className="text-sky-400">uv run uvicorn app.main:app</code> in{' '}
                <code className="text-slate-300">backend/</code>
              </p>
              {error && (
                <p role="alert" className="text-xs text-rose-400 font-mono break-all">
                  {error.kind !== 'unknown' && (
                    <span className="uppercase text-rose-300">[{error.kind}] </span>
                  )}
                  {error.message}
                </p>
              )}
            </>
          )}

          <p className="text-xs text-slate-500">
            Last successful check:{' '}
            {lastSuccessAt ? (
              <time dateTime={lastSuccessAt.toISOString()} className="font-mono text-slate-400">
                {formatTime(lastSuccessAt)}
              </time>
            ) : (
              <span className="font-mono text-slate-400">Never</span>
            )}
            {' · '}Auto-checks every {Math.round(pollIntervalMs / 1000)}s
          </p>
        </div>
      </div>
    </section>
  );
};
