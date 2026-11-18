import { useCallback, useEffect, useRef, useState } from 'react';
import { fetchHealth } from '../services/api';
import { getErrorMessage, isApiError, type ApiErrorKind } from '../services/apiError';
import type { HealthCheckResponse } from '../types';

/** Default delay between background health checks. */
export const DEFAULT_HEALTH_POLL_INTERVAL_MS = 15_000;

export type ConnectionStatus = 'checking' | 'connected' | 'offline';

export interface BackendHealthError {
  message: string;
  kind: ApiErrorKind | 'unknown';
}

export interface BackendHealthState {
  /**
   * `checking` before the first result and during a manual retry. Background
   * polls keep the previous status until they settle to avoid UI flicker.
   */
  status: ConnectionStatus;
  /** Payload of the most recent successful check (kept while offline). */
  data: HealthCheckResponse | null;
  /** Error from the most recent failed check; cleared on success. */
  error: BackendHealthError | null;
  /** When the backend last answered successfully. */
  lastSuccessAt: Date | null;
  /** When the most recent check (success or failure) completed. */
  lastCheckedAt: Date | null;
  /** True while a request is in flight. */
  isChecking: boolean;
}

export interface UseBackendHealthResult extends BackendHealthState {
  /** Run a check now and restart the polling interval. No-op while a check is in flight. */
  retry: () => void;
}

export interface UseBackendHealthOptions {
  /** Delay between background checks, measured from the end of the previous check. */
  intervalMs?: number;
  /** Per-request timeout passed to `fetchHealth`. Defaults to the API client's default. */
  timeoutMs?: number;
}

type CheckTrigger = 'initial' | 'manual' | 'poll' | 'visible';

const INITIAL_STATE: BackendHealthState = {
  status: 'checking',
  data: null,
  error: null,
  lastSuccessAt: null,
  lastCheckedAt: null,
  isChecking: false,
};

function isDocumentHidden(): boolean {
  return typeof document !== 'undefined' && document.visibilityState === 'hidden';
}

/**
 * Monitor backend health:
 * - checks immediately on mount, then every `intervalMs` after each check settles,
 *   so requests never overlap;
 * - pauses polling while the tab is hidden and checks immediately when it becomes visible;
 * - aborts the in-flight request and stops all timers on unmount (StrictMode-safe).
 */
export function useBackendHealth(options: UseBackendHealthOptions = {}): UseBackendHealthResult {
  const { intervalMs = DEFAULT_HEALTH_POLL_INTERVAL_MS, timeoutMs } = options;

  if (!Number.isFinite(intervalMs) || intervalMs <= 0) {
    throw new RangeError(`intervalMs must be a positive finite number, received ${intervalMs}.`);
  }

  const [state, setState] = useState<BackendHealthState>(INITIAL_STATE);
  const retryRef = useRef<() => void>(() => {});

  useEffect(() => {
    // Everything below is scoped to this effect run. Under StrictMode the first
    // run is disposed (request aborted, timers cleared) before the second starts.
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let inFlight: AbortController | null = null;

    const clearTimer = (): void => {
      if (timer !== undefined) {
        clearTimeout(timer);
        timer = undefined;
      }
    };

    const scheduleNext = (): void => {
      clearTimer();
      if (disposed || isDocumentHidden()) {
        return; // Paused; resumed by the visibilitychange handler.
      }
      timer = setTimeout(() => {
        timer = undefined;
        void runCheck('poll');
      }, intervalMs);
    };

    const runCheck = async (trigger: CheckTrigger): Promise<void> => {
      if (disposed || inFlight !== null) {
        return; // Never start overlapping requests.
      }
      clearTimer();

      const controller = new AbortController();
      inFlight = controller;
      const showChecking = trigger === 'initial' || trigger === 'manual';
      setState((prev) => ({
        ...prev,
        status: showChecking ? 'checking' : prev.status,
        isChecking: true,
      }));

      try {
        const data = await fetchHealth({ signal: controller.signal, timeoutMs });
        if (disposed) return;
        const now = new Date();
        setState((prev) => ({
          ...prev,
          status: 'connected',
          data,
          error: null,
          lastSuccessAt: now,
          lastCheckedAt: now,
          isChecking: false,
        }));
      } catch (error: unknown) {
        // Aborts only happen on dispose; any other failure is reported.
        if (disposed) return;
        setState((prev) => ({
          ...prev,
          status: 'offline',
          error: {
            message: getErrorMessage(error, 'Backend unreachable.'),
            kind: isApiError(error) ? error.kind : 'unknown',
          },
          lastCheckedAt: new Date(),
          isChecking: false,
        }));
      } finally {
        if (inFlight === controller) {
          inFlight = null;
        }
        scheduleNext();
      }
    };

    const handleVisibilityChange = (): void => {
      if (isDocumentHidden()) {
        clearTimer();
      } else {
        void runCheck('visible');
      }
    };

    retryRef.current = () => {
      void runCheck('manual');
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    void runCheck('initial');

    return () => {
      disposed = true;
      clearTimer();
      inFlight?.abort();
      inFlight = null;
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      retryRef.current = () => {};
    };
  }, [intervalMs, timeoutMs]);

  const retry = useCallback(() => retryRef.current(), []);

  return { ...state, retry };
}
