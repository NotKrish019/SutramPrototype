import { act, render, type RenderResult } from '@testing-library/react';
import { vi } from 'vitest';
import App from '../App';

export type HealthMode = 'ok' | 'down' | 'hang';

/** Stub `fetch` for the backend health endpoint. */
export function stubHealth(mode: HealthMode = 'ok') {
  const fetchMock = vi.fn(async (_input: string, init?: RequestInit) => {
    if (mode === 'down') throw new TypeError('Failed to fetch');
    if (mode === 'hang') {
      return new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')));
      });
    }
    return new Response(JSON.stringify({ status: 'ok', project: 'sutram-digital-forensics' }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
  });
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}

/** Render the whole app at a path (pathname + optional query). */
export async function renderAppAt(path: string): Promise<RenderResult> {
  window.history.replaceState(null, '', path);
  let result!: RenderResult;
  await act(async () => {
    result = render(<App />);
  });
  // Let the synthetic repository (microtask) and first health check settle.
  await act(async () => {
    await Promise.resolve();
  });
  return result;
}
