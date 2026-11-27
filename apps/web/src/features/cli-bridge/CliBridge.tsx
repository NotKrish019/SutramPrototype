/**
 * The web ↔ terminal bridge.
 *
 * The workspace and the `yukti` CLI are two clients of the same Rust API. This module makes that
 * visible: a dock that shows the CLI command for the view on screen, and a watcher that announces
 * cases created from the terminal while the page is open.
 */
import React, { useEffect, useRef, useState } from 'react';
import { MonitorPlay, Terminal, X } from 'lucide-react';
import { Link } from '../../router/Link';
import { caseHref, type CaseSection } from '../../router/router';
import { fetchCases, type CaseRecordDto } from '../../services/casesApi';
import { getApiBaseUrl } from '../../services/config';
import { useWorkspaceRepository } from '../workspace/data/WorkspaceDataContext';
import { useWorkspaceShell } from '../workspace/layout/WorkspaceShellContext';
import { CliCommand } from './CliCommand';

export const LIVE_POLL_MS = 5000;

/** The CLI commands that reproduce a workspace view for a case. */
export function cliCommandsFor(section: CaseSection, caseId: string): { command: string; what: string }[] {
  const c = `--case ${caseId}`;
  const open = { command: `yukti open ${c} --view ${section}`, what: 'Open this exact view from the terminal' };
  const map: Record<CaseSection, { command: string; what: string }[]> = {
    overview: [
      { command: `yukti case overview ${c}`, what: 'One-screen investigative brief' },
      { command: `yukti ask "who was near the gate after 22:30?" ${c}`, what: 'Plain-English question, answered with citations' },
    ],
    theatre: [
      { command: `yukti replay ${c}`, what: 'Animated replay with a movement map, in the terminal' },
      { command: `yukti challenge ${c}`, what: 'Defence-counsel view: weakest points first' },
    ],
    evidence: [
      { command: `yukti evidence list ${c}`, what: 'Exhibits with SHA-256 and ingestion status' },
      { command: `yukti evidence ingest <file> ${c} --auto-extract`, what: 'Ingest and extract a new exhibit' },
      { command: `yukti custody verify ${c}`, what: 'Re-hash every exhibit and print the case seal' },
    ],
    timeline: [
      { command: `yukti timeline ${c}`, what: 'Normalised timeline with uncertainty windows' },
      { command: `yukti whatif --clock-offset 120 ${c}`, what: 'What if one clock was 2 minutes off?' },
    ],
    spatial: [{ command: `yukti spatial ${c}`, what: 'Trajectories and speed-threshold checks' }],
    contradictions: [
      { command: `yukti contradictions ${c}`, what: 'Contradiction leads with their basis' },
      { command: `yukti robustness ${c}`, what: 'How wrong an assumption must be to dissolve each lead' },
    ],
    video: [{ command: `yukti evidence list ${c}`, what: 'Video exhibits and frame provenance' }],
    reports: [
      { command: `yukti report ${c} --out dossier.md`, what: 'Export the forensic dossier' },
      { command: `yukti certificate ${c} --out section63.md`, what: 'Draft the Section 63 BSA certificate' },
    ],
    graph: [{ command: `yukti graph summary ${c}`, what: 'Graph counts and sync status' }],
    correlation: [{ command: `yukti provenance ${c}`, what: 'Trace findings back to exhibits and hashes' }],
  };
  return [...map[section], open];
}

/* ------------------------------------------------------------------ live watcher */

interface Arrival {
  record: CaseRecordDto;
  at: number;
}

/**
 * Polls the API's case list while live and reports cases that were not there before (typically
 * created with `yukti case create` or `yukti demo`). The first poll only records a baseline.
 */
function useCaseArrivals(enabled: boolean, onChange: () => void): [Arrival[], (id: string) => void] {
  const known = useRef<Set<string> | null>(null);
  const [arrivals, setArrivals] = useState<Arrival[]>([]);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!enabled) {
      known.current = null;
      return;
    }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const controller = new AbortController();
    const poll = async (): Promise<void> => {
      if (typeof document !== 'undefined' && document.visibilityState === 'hidden') {
        timer = setTimeout(() => void poll(), LIVE_POLL_MS);
        return;
      }
      try {
        const records = await fetchCases({ signal: controller.signal, timeoutMs: 4000 });
        if (cancelled) return;
        if (known.current === null) {
          known.current = new Set(records.map((r) => r.case_id));
        } else {
          const fresh = records.filter((r) => !known.current!.has(r.case_id));
          if (fresh.length) {
            fresh.forEach((r) => known.current!.add(r.case_id));
            setArrivals((list) => [...fresh.map((record) => ({ record, at: Date.now() })), ...list].slice(0, 3));
            onChangeRef.current();
          }
        }
      } catch {
        // Offline or aborted: the health monitor reports connectivity; try again on the next tick.
      }
      if (!cancelled) timer = setTimeout(() => void poll(), LIVE_POLL_MS);
    };
    void poll();
    return () => {
      cancelled = true;
      controller.abort();
      if (timer) clearTimeout(timer);
    };
  }, [enabled]);

  const dismiss = (id: string): void => setArrivals((list) => list.filter((a) => a.record.case_id !== id));
  return [arrivals, dismiss];
}

const ArrivalToast: React.FC<{ arrival: Arrival; onDismiss: () => void }> = ({ arrival, onDismiss }) => {
  useEffect(() => {
    const timer = setTimeout(onDismiss, 15_000);
    return () => clearTimeout(timer);
  }, [onDismiss]);
  const { record } = arrival;
  return (
    <div role="status" className="bridge-toast pointer-events-auto w-[22rem] border border-crimson/60 bg-ink-900/95 shadow-flag backdrop-blur-md">
      <div className="flex items-start gap-3 p-3.5">
        <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center border border-crimson/50 bg-crimson/10 text-crimson">
          <Terminal aria-hidden="true" className="h-4 w-4" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="font-mono text-micro uppercase text-crimson">New case from the terminal</p>
          <p className="mt-1 truncate font-serif text-base text-fg-bright">{record.title}</p>
          <p className="truncate font-mono text-[0.65rem] text-fg-subtle">
            {record.fir_number ? `${record.fir_number} · ` : ''}
            {record.case_id}
          </p>
          <div className="mt-2.5 flex gap-2">
            <Link to={caseHref(record.case_id)} onClick={onDismiss} className="inline-flex h-8 items-center border border-ink-strong px-2.5 font-mono text-[0.65rem] uppercase tracking-wider text-fg hover:border-fg-subtle">
              Open
            </Link>
            <Link
              to={caseHref(record.case_id, 'theatre')}
              onClick={onDismiss}
              className="inline-flex h-8 items-center gap-1.5 bg-crimson px-2.5 font-mono text-[0.65rem] uppercase tracking-wider text-white hover:bg-crimson-deep"
            >
              <MonitorPlay aria-hidden="true" className="h-3.5 w-3.5" /> Play
            </Link>
          </div>
        </div>
        <button type="button" onClick={onDismiss} aria-label="Dismiss" className="text-fg-subtle hover:text-fg">
          <X aria-hidden="true" className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
};

/* ------------------------------------------------------------------ dock */

const TerminalDock: React.FC<{ caseId: string; section: CaseSection; live: boolean }> = ({ caseId, section, live }) => {
  const [open, setOpen] = useState(false);
  const commands = live ? cliCommandsFor(section, caseId) : [{ command: 'yukti demo', what: 'Builds a live case on the local engine; it appears here within seconds' }];
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open]);

  return (
    <div className="pointer-events-auto flex flex-col items-end gap-2">
      {open && (
        <section aria-label="Terminal equivalents" className="bridge-dock w-[min(30rem,calc(100vw-2rem))] border border-ink-strong bg-ink-950/95 shadow-lift backdrop-blur-md">
          <header className="flex items-center gap-2 border-b border-ink-line px-3 py-2">
            <span className="flex gap-1" aria-hidden="true">
              <span className="h-2.5 w-2.5 rounded-full bg-crimson" />
              <span className="h-2.5 w-2.5 rounded-full bg-ink-strong" />
              <span className="h-2.5 w-2.5 rounded-full bg-ink-strong" />
            </span>
            <h2 className="flex-1 font-mono text-micro uppercase text-fg-muted">yukti · same view in the terminal</h2>
            <button type="button" onClick={() => setOpen(false)} aria-label="Close terminal panel" className="text-fg-subtle hover:text-fg">
              <X aria-hidden="true" className="h-4 w-4" />
            </button>
          </header>
          <p className="flex items-center gap-2 border-b border-ink-line px-3 py-2 font-mono text-[0.68rem] text-fg-subtle">
            <span className={`h-1.5 w-1.5 rounded-full ${live ? 'bg-signal-ok' : 'bg-signal-warn'}`} aria-hidden="true" />
            {live ? (
              <>
                Linked · web and CLI read <span className="text-fg">{getApiBaseUrl()}</span>
              </>
            ) : (
              'Synthetic case · start the engine with the CLI to go live'
            )}
          </p>
          <ul className="space-y-2.5 p-3">
            {commands.map((item) => (
              <li key={item.command}>
                <p className="mb-1 text-xs text-fg-muted">{item.what}</p>
                <CliCommand command={item.command} compact />
              </li>
            ))}
          </ul>
        </section>
      )}
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="group inline-flex h-11 items-center gap-2 border border-ink-strong bg-ink-950/90 pl-3 pr-3.5 font-mono text-micro uppercase text-fg shadow-lift backdrop-blur-md transition-colors hover:border-crimson/70 focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent"
      >
        <Terminal aria-hidden="true" className="h-4 w-4 text-crimson" />
        Terminal
        <span className={`h-1.5 w-1.5 rounded-full ${live ? 'bg-signal-ok bridge-pulse' : 'bg-signal-warn'}`} aria-hidden="true" />
      </button>
    </div>
  );
};

/** Mounted once by the workspace shell. */
export const CliBridge: React.FC = () => {
  const repository = useWorkspaceRepository();
  const { route, cases } = useWorkspaceShell();
  const live = repository.source === 'api';
  const [arrivals, dismiss] = useCaseArrivals(live, cases.refresh);
  const caseItem =
    route.kind === 'case' && cases.state.status === 'success' ? cases.state.data.find((c) => c.case_id === route.caseId) : undefined;
  const caseLive = live && (caseItem?.source ?? repository.source) === 'api';

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-50 flex flex-col items-end gap-3 sm:bottom-6 sm:right-6">
      {arrivals.map((a) => (
        <ArrivalToast key={a.record.case_id} arrival={a} onDismiss={() => dismiss(a.record.case_id)} />
      ))}
      {route.kind === 'case' && route.section !== 'theatre' && (
        <TerminalDock caseId={route.caseId} section={route.section} live={caseLive} />
      )}
    </div>
  );
};
