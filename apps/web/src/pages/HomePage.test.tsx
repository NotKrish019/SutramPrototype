import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { HomePage } from './HomePage';

const BASE = 'http://backend.test:9000';

beforeEach(() => {
  vi.stubEnv('VITE_API_URL', BASE);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
});

describe('HomePage backend telemetry', () => {
  it('shows the configured backend URL while loading', () => {
    vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(() => {})));
    render(<HomePage />);
    expect(screen.getByText(`Pinging backend at ${BASE}/health...`)).toBeInTheDocument();
    expect(screen.queryByText(/localhost:8000/)).not.toBeInTheDocument();
  });

  it('shows connected status on success', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(JSON.stringify({ status: 'ok', project: 'sutram-digital-forensics' }))),
    );
    render(<HomePage />);
    expect(
      await screen.findByText('Connected: sutram-digital-forensics (Status: ok)'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
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
    render(<HomePage />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'HTTP 503 Service Unavailable: Database unavailable.',
    );
    expect(screen.getByText(/Backend Status: Offline/)).toBeInTheDocument();
  });

  it('shows a network error message when the backend is unreachable', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))));
    render(<HomePage />);
    expect(await screen.findByRole('alert')).toHaveTextContent(/Network error while contacting/);
  });

  it('shows a parse error message for malformed payloads', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ status: 'ok' }))));
    render(<HomePage />);
    expect(await screen.findByRole('alert')).toHaveTextContent(/Unexpected response shape/);
  });

  it('handles an invalid VITE_API_URL without crashing', async () => {
    vi.stubEnv('VITE_API_URL', 'not a url');
    vi.stubGlobal('fetch', vi.fn());
    render(<HomePage />);
    expect(await screen.findByRole('alert')).toHaveTextContent(/Invalid VITE_API_URL/);
  });

  it('aborts the in-flight request on unmount', () => {
    const fetchMock = vi.fn((_input: string, _init?: RequestInit) => new Promise<Response>(() => {}));
    vi.stubGlobal('fetch', fetchMock);
    const { unmount } = render(<HomePage />);
    const signal = fetchMock.mock.calls[0]![1]?.signal;
    expect(signal?.aborted).toBe(false);
    unmount();
    expect(signal?.aborted).toBe(true);
  });
});
