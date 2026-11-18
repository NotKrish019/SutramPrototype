import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, screen, within } from '@testing-library/react';
import { renderAppAt } from '../test/renderApp';

const BASE = 'http://backend.test:9000';

const healthyResponse = (): Response =>
  new Response(JSON.stringify({ status: 'ok', project: 'sutram-digital-forensics' }));

/** The restyled BackendStatusCard on the workspace overview. */
const card = (): HTMLElement => screen.getByRole('region', { name: 'Backend Gateway Telemetry' });

beforeEach(() => {
  vi.stubEnv('VITE_API_URL', BASE);
  vi.spyOn(window, 'scrollTo').mockImplementation(() => {});
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  window.history.replaceState(null, '', '/');
});

describe('Workspace overview — backend telemetry (integration)', () => {
  it('shows the checking state with the configured backend URL', async () => {
    vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(() => {})));
    await renderAppAt('/workspace');
    expect(within(card()).getByRole('status')).toHaveTextContent('Checking');
    expect(within(card()).getByText(`Pinging backend at ${BASE}/health...`)).toBeInTheDocument();
    expect(screen.queryByText(/localhost:8000/)).not.toBeInTheDocument();
    expect(within(card()).getByRole('button', { name: 'Retry Now' })).toBeDisabled();
  });

  it('shows connected status and a last successful check time', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => healthyResponse()));
    await renderAppAt('/workspace');
    expect(await within(card()).findByText('sutram-digital-forensics (Status: ok)')).toBeInTheDocument();
    expect(within(card()).getByRole('status')).toHaveTextContent('Connected');
    expect(card().querySelector('time')).not.toBeNull();
    expect(within(card()).queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows offline status with the API error message on HTTP failure', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          new Response(JSON.stringify({ error: { code: 'X', message: 'Database unavailable.' } }), {
            status: 503,
            statusText: 'Service Unavailable',
          }),
      ),
    );
    await renderAppAt('/workspace');
    expect(await within(card()).findByRole('alert')).toHaveTextContent('[http] HTTP 503 Service Unavailable: Database unavailable.');
    expect(within(card()).getByRole('status')).toHaveTextContent('Offline');
    expect(within(card()).getByText('Never')).toBeInTheDocument();
  });

  it('shows a network error message when the backend is unreachable', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))));
    await renderAppAt('/workspace');
    expect(await within(card()).findByRole('alert')).toHaveTextContent(/\[network\] Network error while contacting/);
  });

  it('shows a parse error message for malformed payloads', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ status: 'ok' }))));
    await renderAppAt('/workspace');
    expect(await within(card()).findByRole('alert')).toHaveTextContent(/\[parse\] Unexpected response shape/);
  });

  it('handles an invalid VITE_API_URL without crashing', async () => {
    vi.stubEnv('VITE_API_URL', 'not a url');
    vi.stubGlobal('fetch', vi.fn());
    await renderAppAt('/workspace');
    expect(await within(card()).findByRole('alert')).toHaveTextContent(/Invalid VITE_API_URL/);
  });

  it('recovers via "Retry Now" without a page refresh', async () => {
    const fetchMock = vi
      .fn()
      .mockRejectedValueOnce(new TypeError('Failed to fetch'))
      .mockImplementation(async () => healthyResponse());
    vi.stubGlobal('fetch', fetchMock);
    await renderAppAt('/workspace');
    expect(await within(card()).findByRole('alert')).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(within(card()).getByRole('button', { name: 'Retry Now' }));
    });

    expect(await within(card()).findByText('sutram-digital-forensics (Status: ok)')).toBeInTheDocument();
    expect(within(card()).getByRole('status')).toHaveTextContent('Connected');
    // One shared monitor for the whole workspace: exactly one initial check + one retry.
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('aborts the in-flight request on unmount', async () => {
    const fetchMock = vi.fn((_input: string, _init?: RequestInit) => new Promise<Response>(() => {}));
    vi.stubGlobal('fetch', fetchMock);
    await renderAppAt('/workspace');
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const signal = fetchMock.mock.calls[0]![1]?.signal;
    expect(signal?.aborted).toBe(false);
    cleanup();
    expect(signal?.aborted).toBe(true);
  });
});
