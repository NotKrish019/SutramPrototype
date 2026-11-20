import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import App from './App';

function healthyFetch() {
  return vi.fn().mockImplementation(() =>
    Promise.resolve(
      new Response(JSON.stringify({ status: 'ok', project: 'sutram-digital-forensics' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    ),
  );
}

async function renderAt(path: string) {
  window.history.replaceState(null, '', path);
  await act(async () => {
    render(<App />);
  });
}

describe('App routing', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', healthyFetch());
    vi.spyOn(window, 'scrollTo').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    window.history.replaceState(null, '', '/');
  });

  it('renders the public product site at /', async () => {
    await renderAt('/');
    expect(screen.getByRole('heading', { level: 1, name: /every fragment tells a story/i })).toBeInTheDocument();
    expect(document.title).toMatch(/YUKTI — Digital Evidence Intelligence/);
  });

  it('renders the redesigned investigator workspace at /workspace', async () => {
    await renderAt('/workspace');

    expect(screen.getByRole('heading', { level: 1, name: 'Investigation Workspace' })).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Workspace' })).toBeInTheDocument();

    // Existing backend health monitoring is still mounted.
    expect(screen.getByText('Backend Gateway Telemetry')).toBeInTheDocument();
    expect(await screen.findByText('sutram-digital-forensics (Status: ok)')).toBeInTheDocument();
    expect(document.title).toBe('Workspace · YUKTI Digital Forensics');
  });

  it('treats a trailing slash as the same route', async () => {
    await renderAt('/workspace/');
    expect(screen.getByText('Backend Gateway Telemetry')).toBeInTheDocument();
  });

  it('routes workspace case sections and sets descriptive titles', async () => {
    await renderAt('/workspace/cases/CASE-026/timeline');
    expect(screen.getByRole('heading', { level: 1, name: 'Warehouse Access Investigation' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /timeline/i, current: 'page' })).toBeInTheDocument();
    expect(document.title).toBe('Temporal Reconstruction · CASE-026 · YUKTI');
  });

  it('shows a workspace-level not-found view for unknown workspace paths', async () => {
    await renderAt('/workspace/nope');
    expect(screen.getByText('No workspace view at this path')).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Workspace' })).toBeInTheDocument();
  });

  it('renders a not-found page with recovery links for unknown paths', async () => {
    await renderAt('/does-not-exist');
    expect(screen.getByRole('heading', { name: /no route at this path/i })).toBeInTheDocument();
    expect(screen.getByText('/does-not-exist')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /back to yukti/i })).toHaveAttribute('href', '/');
  });

  it('navigates client-side from the site to the workspace and back with history', async () => {
    await renderAt('/');
    const [openWorkspace] = screen.getAllByRole('link', { name: 'Open Workspace' });

    await act(async () => {
      fireEvent.click(openWorkspace!);
    });
    expect(window.location.pathname).toBe('/workspace');
    expect(screen.getByText('Backend Gateway Telemetry')).toBeInTheDocument();

    // The workspace sidebar links back to the public site.
    await act(async () => {
      fireEvent.click(screen.getByRole('link', { name: 'Public site' }));
    });
    expect(window.location.pathname).toBe('/');
    expect(screen.getByRole('heading', { level: 1, name: /every fragment tells a story/i })).toBeInTheDocument();

    // Browser back button.
    await act(async () => {
      window.history.pushState(null, '', '/workspace');
      window.dispatchEvent(new PopStateEvent('popstate'));
    });
    expect(screen.getByText('Backend Gateway Telemetry')).toBeInTheDocument();
  });
});
