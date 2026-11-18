import React, { StrictMode } from 'react';
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fetchHealth } from '../services/api';
import { ApiError } from '../services/apiError';
import type { HealthCheckResponse } from '../types';
import { DEFAULT_HEALTH_POLL_INTERVAL_MS, useBackendHealth } from './useBackendHealth';

vi.mock('../services/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../services/api')>()),
  fetchHealth: vi.fn(),
}));

const fetchHealthMock = vi.mocked(fetchHealth);
const INTERVAL = DEFAULT_HEALTH_POLL_INTERVAL_MS;
const HEALTHY: HealthCheckResponse = { status: 'ok', project: 'sutram-digital-forensics' };
const T0 = new Date('2026-11-18T10:00:00.000Z');

const networkError = (): ApiError =>
  new ApiError({ kind: 'network', message: 'Network error while contacting backend.', url: 'u' });

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** Behaves like the real client: rejects with an AbortError when the signal fires. */
function pendingUntilAborted(): (opts?: { signal?: AbortSignal }) => Promise<HealthCheckResponse> {
  return (opts) =>
    new Promise<HealthCheckResponse>((_resolve, reject) => {
      opts?.signal?.addEventListener('abort', () =>
        reject(new DOMException('The operation was aborted.', 'AbortError')),
      );
    });
}

let visibility: DocumentVisibilityState = 'visible';

function setVisibility(next: DocumentVisibilityState): void {
  visibility = next;
  act(() => {
    document.dispatchEvent(new Event('visibilitychange'));
  });
}

/** Flush resolved promises and React updates without advancing the clock. */
async function flush(): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

async function advance(ms: number): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

function signalOfCall(index: number): AbortSignal | undefined {
  return fetchHealthMock.mock.calls[index]?.[0]?.signal;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(T0);
  visibility = 'visible';
  Object.defineProperty(document, 'visibilityState', {
    configurable: true,
    get: () => visibility,
  });
  fetchHealthMock.mockReset();
});

afterEach(() => {
  Reflect.deleteProperty(document, 'visibilityState');
  vi.useRealTimers();
});

describe('useBackendHealth', () => {
  it('starts in the checking state and checks immediately on mount', () => {
    fetchHealthMock.mockImplementation(pendingUntilAborted());
    const { result } = renderHook(() => useBackendHealth());

    expect(fetchHealthMock).toHaveBeenCalledTimes(1);
    expect(result.current).toMatchObject({
      status: 'checking',
      isChecking: true,
      data: null,
      error: null,
      lastSuccessAt: null,
    });
  });

  it('1. backend initially online → connected with last-success timestamp', async () => {
    fetchHealthMock.mockResolvedValue(HEALTHY);
    const { result } = renderHook(() => useBackendHealth());
    await flush();

    expect(result.current).toMatchObject({
      status: 'connected',
      data: HEALTHY,
      error: null,
      isChecking: false,
    });
    expect(result.current.lastSuccessAt).toEqual(T0);
    expect(result.current.lastCheckedAt).toEqual(T0);
  });

  it('2. backend initially offline → offline with typed network error', async () => {
    fetchHealthMock.mockRejectedValue(networkError());
    const { result } = renderHook(() => useBackendHealth());
    await flush();

    expect(result.current).toMatchObject({
      status: 'offline',
      data: null,
      lastSuccessAt: null,
      isChecking: false,
      error: { kind: 'network', message: 'Network error while contacting backend.' },
    });
    expect(result.current.lastCheckedAt).toEqual(T0);
  });

  it('3. backend goes offline after initial success → offline, keeps last success time', async () => {
    fetchHealthMock.mockResolvedValueOnce(HEALTHY).mockRejectedValueOnce(networkError());
    const { result } = renderHook(() => useBackendHealth());
    await flush();
    expect(result.current.status).toBe('connected');

    await advance(INTERVAL);

    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
    expect(result.current.status).toBe('offline');
    expect(result.current.error?.kind).toBe('network');
    expect(result.current.lastSuccessAt).toEqual(T0);
    expect(result.current.lastCheckedAt).toEqual(new Date(T0.getTime() + INTERVAL));
  });

  it('background polls do not flash the checking state', async () => {
    fetchHealthMock.mockResolvedValueOnce(HEALTHY).mockImplementationOnce(pendingUntilAborted());
    const { result } = renderHook(() => useBackendHealth());
    await flush();
    await advance(INTERVAL);

    expect(result.current.isChecking).toBe(true);
    expect(result.current.status).toBe('connected');
  });

  it('4. backend recovers without a page refresh', async () => {
    fetchHealthMock
      .mockRejectedValueOnce(networkError())
      .mockRejectedValueOnce(networkError())
      .mockResolvedValueOnce(HEALTHY);
    const { result } = renderHook(() => useBackendHealth());
    await flush();
    expect(result.current.status).toBe('offline');

    await advance(INTERVAL);
    expect(result.current.status).toBe('offline');

    await advance(INTERVAL);
    expect(fetchHealthMock).toHaveBeenCalledTimes(3);
    expect(result.current).toMatchObject({ status: 'connected', error: null, data: HEALTHY });
    expect(result.current.lastSuccessAt).toEqual(new Date(T0.getTime() + 2 * INTERVAL));
  });

  it('polls exactly once per interval', async () => {
    fetchHealthMock.mockResolvedValue(HEALTHY);
    renderHook(() => useBackendHealth());
    await flush();

    await advance(INTERVAL - 1);
    expect(fetchHealthMock).toHaveBeenCalledTimes(1);
    await advance(1);
    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
    await advance(INTERVAL * 3);
    expect(fetchHealthMock).toHaveBeenCalledTimes(5);
  });

  it('5. manual retry checks immediately, shows checking, and restarts the interval', async () => {
    const retryResult = deferred<HealthCheckResponse>();
    fetchHealthMock
      .mockRejectedValueOnce(networkError())
      .mockReturnValueOnce(retryResult.promise)
      .mockResolvedValue(HEALTHY);
    const { result } = renderHook(() => useBackendHealth());
    await flush();
    expect(result.current.status).toBe('offline');

    await advance(5_000);
    act(() => result.current.retry());

    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
    expect(result.current).toMatchObject({ status: 'checking', isChecking: true });

    retryResult.resolve(HEALTHY);
    await flush();
    expect(result.current.status).toBe('connected');

    // Next poll is a full interval after the retry, not after the original schedule.
    await advance(INTERVAL - 5_000);
    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
    await advance(5_000);
    expect(fetchHealthMock).toHaveBeenCalledTimes(3);
  });

  it('keeps the retry callback stable across renders', async () => {
    fetchHealthMock.mockResolvedValue(HEALTHY);
    const { result } = renderHook(() => useBackendHealth());
    const firstRetry = result.current.retry;
    await flush();
    expect(result.current.retry).toBe(firstRetry);
  });

  it('6. pauses background polling while the tab is hidden', async () => {
    fetchHealthMock.mockResolvedValue(HEALTHY);
    renderHook(() => useBackendHealth());
    await flush();

    setVisibility('hidden');
    await advance(INTERVAL * 5);

    expect(fetchHealthMock).toHaveBeenCalledTimes(1);
  });

  it('does not schedule a poll when a check settles while the tab is hidden', async () => {
    const first = deferred<HealthCheckResponse>();
    fetchHealthMock.mockReturnValueOnce(first.promise).mockResolvedValue(HEALTHY);
    renderHook(() => useBackendHealth());

    setVisibility('hidden');
    first.resolve(HEALTHY);
    await flush();
    await advance(INTERVAL * 3);

    expect(fetchHealthMock).toHaveBeenCalledTimes(1);
  });

  it('7. checks immediately when the tab becomes visible and then resumes polling', async () => {
    fetchHealthMock.mockResolvedValueOnce(HEALTHY).mockRejectedValueOnce(networkError());
    fetchHealthMock.mockResolvedValue(HEALTHY);
    const { result } = renderHook(() => useBackendHealth());
    await flush();

    setVisibility('hidden');
    await advance(INTERVAL * 2);
    expect(fetchHealthMock).toHaveBeenCalledTimes(1);

    setVisibility('visible');
    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
    await flush();
    expect(result.current.status).toBe('offline');

    await advance(INTERVAL);
    expect(fetchHealthMock).toHaveBeenCalledTimes(3);
    expect(result.current.status).toBe('connected');
  });

  it('8. unmount aborts the pending request and stops polling', async () => {
    fetchHealthMock.mockImplementation(pendingUntilAborted());
    const { unmount } = renderHook(() => useBackendHealth());
    const signal = signalOfCall(0);
    expect(signal?.aborted).toBe(false);

    unmount();
    expect(signal?.aborted).toBe(true);

    await advance(INTERVAL * 3);
    setVisibility('visible');
    expect(fetchHealthMock).toHaveBeenCalledTimes(1);
  });

  it('ignores a late response from a disposed StrictMode effect run', async () => {
    // First (disposed) run's request ignores abort and resolves late with stale data.
    const stale = deferred<HealthCheckResponse>();
    fetchHealthMock.mockReturnValueOnce(stale.promise).mockResolvedValue(HEALTHY);
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <StrictMode>{children}</StrictMode>
    );
    const { result } = renderHook(() => useBackendHealth(), { wrapper });
    await flush();
    expect(result.current.data).toEqual(HEALTHY);

    stale.resolve({ status: 'stale', project: 'stale' });
    await flush();

    expect(result.current.data).toEqual(HEALTHY);
  });

  it('9. never starts overlapping requests', async () => {
    const slow = deferred<HealthCheckResponse>();
    fetchHealthMock.mockReturnValueOnce(slow.promise).mockResolvedValue(HEALTHY);
    const { result } = renderHook(() => useBackendHealth());

    // Poll interval elapsing, manual retries and visibility changes while in flight.
    await advance(INTERVAL * 4);
    act(() => result.current.retry());
    act(() => result.current.retry());
    setVisibility('hidden');
    setVisibility('visible');
    expect(fetchHealthMock).toHaveBeenCalledTimes(1);

    slow.resolve(HEALTHY);
    await flush();
    expect(fetchHealthMock).toHaveBeenCalledTimes(1);

    // The next poll is scheduled only after the slow request settled.
    await advance(INTERVAL);
    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
  });

  it('handles React StrictMode double-mount with a single live request', async () => {
    fetchHealthMock.mockImplementationOnce(pendingUntilAborted()).mockResolvedValue(HEALTHY);
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <StrictMode>{children}</StrictMode>
    );
    const { result } = renderHook(() => useBackendHealth(), { wrapper });

    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
    expect(signalOfCall(0)?.aborted).toBe(true);
    expect(signalOfCall(1)?.aborted).toBe(false);

    await flush();
    expect(result.current.status).toBe('connected');

    // Only the surviving effect polls.
    await advance(INTERVAL);
    expect(fetchHealthMock).toHaveBeenCalledTimes(3);
  });

  it('handles unknown (non-ApiError) failures safely', async () => {
    fetchHealthMock.mockRejectedValue('weird failure');
    const { result } = renderHook(() => useBackendHealth());
    await flush();
    expect(result.current.error).toEqual({ kind: 'unknown', message: 'weird failure' });

    fetchHealthMock.mockRejectedValue(undefined);
    await advance(INTERVAL);
    expect(result.current.error).toEqual({ kind: 'unknown', message: 'Backend unreachable.' });
  });

  it('passes the timeout through and honours a custom interval', async () => {
    fetchHealthMock.mockResolvedValue(HEALTHY);
    renderHook(() => useBackendHealth({ intervalMs: 1_000, timeoutMs: 2_500 }));
    await flush();
    expect(fetchHealthMock.mock.calls[0]?.[0]).toMatchObject({ timeoutMs: 2_500 });

    await advance(1_000);
    expect(fetchHealthMock).toHaveBeenCalledTimes(2);
  });

  it('rejects an invalid interval', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => renderHook(() => useBackendHealth({ intervalMs: 0 }))).toThrow(RangeError);
    spy.mockRestore();
  });
});
