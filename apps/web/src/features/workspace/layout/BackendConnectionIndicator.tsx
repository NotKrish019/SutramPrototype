import React, { useEffect, useId, useRef, useState } from 'react';
import { ChevronDown, RefreshCw } from 'lucide-react';
import { HEALTH_PATH } from '../../../services/api';
import { buildApiUrl } from '../../../services/config';
import type { ConnectionStatus } from '../../../hooks/useBackendHealth';
import { useSharedBackendHealth } from '../data/BackendHealthContext';
import { WsButton } from '../components/ui';

const STATUS: Record<ConnectionStatus, { label: string; dot: string; text: string }> = {
  checking: { label: 'Checking', dot: 'bg-signal-accent animate-pulse motion-reduce:animate-none', text: 'text-signal-accent' },
  connected: { label: 'Connected', dot: 'bg-signal-ok', text: 'text-signal-ok' },
  offline: { label: 'Offline', dot: 'bg-signal-flag', text: 'text-signal-flag' },
};

function safeHealthUrl(): string {
  try {
    return buildApiUrl(HEALTH_PATH);
  } catch {
    return 'invalid VITE_API_URL';
  }
}

/** Compact backend status for the top bar, backed by the shared health monitor (real polling + retry). */
export const BackendConnectionIndicator: React.FC = () => {
  const health = useSharedBackendHealth();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();
  const style = STATUS[health.status];

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: PointerEvent): void => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') {
        setOpen(false);
        buttonRef.current?.focus();
      }
    };
    document.addEventListener('pointerdown', onPointer);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('pointerdown', onPointer);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative">
      <button
        ref={buttonRef}
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        aria-label={`Backend API: ${style.label}`}
        onClick={() => setOpen((value) => !value)}
        className="flex h-9 items-center gap-2 border border-ink-line px-2.5 font-mono text-micro uppercase text-fg-muted transition-colors hover:border-ink-strong hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-accent"
      >
        <span aria-hidden="true" className={`h-1.5 w-1.5 ${style.dot}`} />
        <span className="hidden sm:inline">API</span>
        <span className={`hidden md:inline ${style.text}`}>{style.label}</span>
        <ChevronDown aria-hidden="true" className="h-3 w-3" />
      </button>

      <div
        id={panelId}
        hidden={!open}
        className="absolute right-0 top-11 z-50 w-[min(20rem,calc(100vw-2rem))] border border-ink-strong bg-ink-900 shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]"
      >
        <div className="border-b border-ink-line px-4 py-3">
          <p className="font-mono text-micro uppercase text-fg-subtle">Backend API</p>
          <p role="status" aria-live="polite" className={`mt-1 text-sm font-medium ${style.text}`}>
            {style.label}
            {health.isChecking && health.status !== 'checking' && <span className="text-fg-subtle"> · checking…</span>}
          </p>
          <p className="mt-1 break-all font-mono text-micro text-fg-subtle">{safeHealthUrl()}</p>
        </div>
        <dl className="divide-y divide-ink-line text-sm">
          <div className="flex justify-between gap-3 px-4 py-2.5">
            <dt className="font-mono text-micro uppercase text-fg-subtle">Last success</dt>
            <dd className="font-mono text-xs text-fg">
              {health.lastSuccessAt ? (
                <time dateTime={health.lastSuccessAt.toISOString()}>{health.lastSuccessAt.toLocaleTimeString()}</time>
              ) : (
                'Never'
              )}
            </dd>
          </div>
          {health.data && health.status === 'connected' && (
            <div className="flex justify-between gap-3 px-4 py-2.5">
              <dt className="font-mono text-micro uppercase text-fg-subtle">Service</dt>
              <dd className="truncate font-mono text-xs text-fg">{health.data.project}</dd>
            </div>
          )}
          {health.error && health.status === 'offline' && (
            <div className="px-4 py-2.5">
              <dt className="font-mono text-micro uppercase text-signal-flag">{health.error.kind} error</dt>
              <dd className="mt-1 break-words text-xs text-fg-muted">{health.error.message}</dd>
            </div>
          )}
        </dl>
        <div className="flex items-center justify-between gap-3 border-t border-ink-line px-4 py-3">
          <span className="font-mono text-micro uppercase text-fg-subtle">Auto-checks every 15s</span>
          <WsButton size="sm" icon={RefreshCw} onClick={health.retry} disabled={health.isChecking}>
            Retry Now
          </WsButton>
        </div>
      </div>
    </div>
  );
};
