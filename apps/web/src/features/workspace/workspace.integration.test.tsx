import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { renderAppAt, stubHealth } from '../../test/renderApp';
import { matchWorkspaceRoute } from '../../router/router';
import { getApiBaseUrl } from '../../services/config';
import { syntheticRepository, type WorkspaceRepository } from './data/repository';
import { WorkspaceApp } from './WorkspaceApp';

beforeEach(() => {
  stubHealth('ok');
  vi.spyOn(window, 'scrollTo').mockImplementation(() => {});
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  window.localStorage.clear();
  window.history.replaceState(null, '', '/');
});

const sidebar = (): HTMLElement => screen.getAllByRole('navigation', { name: 'Workspace' })[0]!;
const click = async (element: Element): Promise<void> => {
  await act(async () => {
    fireEvent.click(element);
  });
};

async function renderWithRepository(path: string, repository: WorkspaceRepository) {
  window.history.replaceState(null, '', path);
  await act(async () => {
    render(<WorkspaceApp route={matchWorkspaceRoute(path)!} repository={repository} />);
  });
}

/* ------------------------------------------------------------------ Shell */

describe('workspace shell', () => {
  it('links every case section in the sidebar to a real route', async () => {
    await renderAppAt('/workspace');
    const nav = sidebar();
    const expected = ['Investigation Overview', 'Evidence Intake', 'Temporal Reconstruction', 'Spatial Reconstruction', 'Contradiction Intelligence', 'Video Intelligence', 'Judicial Report', 'Knowledge Graph', 'Evidence Correlation'];
    for (const label of expected) {
      const link = within(nav).getByRole('link', { name: new RegExp(`^${label}`) });
      expect(matchWorkspaceRoute(new URL(link.getAttribute('href')!, 'http://x').pathname)?.kind).toBe('case');
    }
    expect(within(nav).getByRole('link', { name: /Documentation/ })).toHaveAttribute('target', '_blank');
    expect(within(nav).getByRole('link', { name: 'Investigations' })).toHaveAttribute('aria-current', 'page');
  });

  it('navigates via the sidebar and marks the active item', async () => {
    await renderAppAt('/workspace');
    await click(within(sidebar()).getByRole('link', { name: /^Knowledge Graph/ }));
    expect(window.location.pathname).toBe('/workspace/cases/CASE-026/graph');
    expect(within(sidebar()).getByRole('link', { name: /^Knowledge Graph/ })).toHaveAttribute('aria-current', 'page');
  });

  it('collapses and expands the sidebar and remembers the choice', async () => {
    await renderAppAt('/workspace');
    await click(screen.getByRole('button', { name: 'Collapse sidebar' }));
    expect(screen.getByRole('button', { name: 'Expand sidebar' })).toHaveAttribute('aria-expanded', 'false');
    expect(within(sidebar()).getByRole('link', { name: 'Knowledge Graph' })).toHaveAttribute('title', 'Knowledge Graph');
    expect(window.localStorage.getItem('yukti.workspace.sidebarCollapsed')).toBe('1');
    await click(screen.getByRole('button', { name: 'Expand sidebar' }));
    expect(window.localStorage.getItem('yukti.workspace.sidebarCollapsed')).toBe('0');
  });

  it('opens the mobile drawer and closes it with Escape, restoring focus', async () => {
    await renderAppAt('/workspace');
    const toggle = screen.getByRole('button', { name: 'Open navigation' });
    await click(toggle);
    const drawer = screen.getByRole('dialog', { name: 'Workspace navigation' });
    expect(within(drawer).getByRole('link', { name: /^Evidence Intake/ })).toBeInTheDocument();
    await act(async () => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(document.activeElement).toBe(toggle);
  });

  it('closes the drawer after choosing a destination', async () => {
    await renderAppAt('/workspace');
    await click(screen.getByRole('button', { name: 'Open navigation' }));
    await click(within(screen.getByRole('dialog')).getByRole('link', { name: /^Judicial Report/ }));
    expect(window.location.pathname).toBe('/workspace/cases/CASE-026/reports');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('shows breadcrumbs and switches case with the case selector, keeping the section', async () => {
    await renderAppAt('/workspace/cases/CASE-026/timeline');
    const crumbs = screen.getByRole('navigation', { name: 'Breadcrumb' });
    expect(crumbs).toHaveTextContent('WorkspaceCASE-026Temporal Reconstruction');
    await act(async () => {
      fireEvent.change(screen.getByRole('combobox', { name: /^Case/ }), { target: { value: 'CASE-024' } });
    });
    expect(window.location.pathname).toBe('/workspace/cases/CASE-024/timeline');
  });

  it('labels synthetic data in the top bar', async () => {
    await renderAppAt('/workspace');
    expect(screen.getByText(/Synthetic demonstration data · case records are fictional/)).toBeInTheDocument();
  });
});

/* -------------------------------------------------------- Backend indicator */

describe('backend connection indicator', () => {
  it('reflects the shared health monitor and retries on demand', async () => {
    const fetchMock = stubHealth('ok');
    await renderAppAt('/workspace/help');
    const button = await screen.findByRole('button', { name: 'Backend API: Connected' });
    await click(button);
    expect(button).toHaveAttribute('aria-expanded', 'true');
    const panel = document.getElementById(button.getAttribute('aria-controls')!)!;
    expect(within(panel).getByText('sutram-digital-forensics')).toBeInTheDocument();
    await click(within(panel).getByRole('button', { name: 'Retry Now' }));
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('shows offline with the typed error — never a hard-coded connected state', async () => {
    stubHealth('down');
    await renderAppAt('/workspace/help');
    const button = await screen.findByRole('button', { name: 'Backend API: Offline' });
    await click(button);
    const panel = document.getElementById(button.getAttribute('aria-controls')!)!;
    expect(within(panel).getByText('network error')).toBeInTheDocument();
    expect(within(panel).getByText('Never')).toBeInTheDocument();
  });

  it('shows checking while the first request is pending', async () => {
    stubHealth('hang');
    await renderAppAt('/workspace/help');
    expect(screen.getByRole('button', { name: 'Backend API: Checking' })).toBeInTheDocument();
  });
});

/* ------------------------------------------------------------------ Search */

describe('workspace search', () => {
  it('finds records across the case and opens the chosen result', async () => {
    await renderAppAt('/workspace');
    const input = screen.getByRole('combobox', { name: /Search the workspace/ });
    await act(async () => {
      fireEvent.change(input, { target: { value: 'badge' } });
    });
    const listbox = screen.getByRole('listbox', { name: 'Search results' });
    expect(within(listbox).getByText('Badge 4471')).toBeInTheDocument();
    expect(within(listbox).getByText('Badge 4471 granted entry')).toBeInTheDocument();
    await act(async () => {
      fireEvent.keyDown(input, { key: 'Enter' });
    });
    expect(window.location.pathname + window.location.search).toMatch(/\/workspace\/cases\/CASE-026\/(evidence|graph|timeline)\?/);
  });

  it('reports no matches and focuses with "/"', async () => {
    await renderAppAt('/workspace');
    const input = screen.getByRole('combobox', { name: /Search the workspace/ });
    await act(async () => {
      document.body.dispatchEvent(new KeyboardEvent('keydown', { key: '/', bubbles: true }));
    });
    expect(document.activeElement).toBe(input);
    await act(async () => {
      fireEvent.change(input, { target: { value: 'zzzz-nothing' } });
    });
    expect(screen.getByText(/No matches for/)).toBeInTheDocument();
  });
});

/* -------------------------------------------------------------- Overview */

describe('workspace overview', () => {
  it('shows portfolio metrics labelled as synthetic and a filterable, sortable case list', async () => {
    await renderAppAt('/workspace');
    const metrics = screen.getByRole('region', { name: 'Case portfolio' });
    expect(within(metrics).getByText('Synthetic demonstration data')).toBeInTheDocument();
    expect(within(metrics).getByText('Total cases').nextSibling).toHaveTextContent('5');
    expect(within(metrics).getByRole('img', { name: 'Case status: 2 Active, 1 Under review, 1 Draft, 1 Archived' })).toBeInTheDocument();

    const table = screen.getAllByRole('table')[0]!;
    expect(within(table).getAllByRole('row')).toHaveLength(6);
    await click(screen.getByRole('button', { name: /^Archived/ }));
    expect(within(table).getAllByRole('row')).toHaveLength(2);
    await click(screen.getByRole('button', { name: /^All/ }));

    await act(async () => {
      fireEvent.change(screen.getByRole('searchbox', { name: 'Search cases' }), { target: { value: 'courier' } });
    });
    expect(within(table).getAllByRole('row')).toHaveLength(2);
    expect(within(table).getByText('Courier Account Takeover')).toBeInTheDocument();
  });

  it('sorts by case id', async () => {
    await renderAppAt('/workspace');
    await act(async () => {
      fireEvent.change(screen.getByRole('combobox', { name: 'Sort' }), { target: { value: 'case' } });
    });
    const rows = within(screen.getAllByRole('table')[0]!).getAllByRole('row').slice(1);
    expect(rows.map((r) => within(r).getAllByText(/^CASE-/)[0]!.textContent)).toEqual(['CASE-011', 'CASE-022', 'CASE-024', 'CASE-026', 'CASE-027']);
  });

  it('does not offer case creation the backend cannot perform', async () => {
    await renderAppAt('/workspace');
    expect(screen.queryByRole('button', { name: /new case|create case/i })).not.toBeInTheDocument();
    expect(screen.getByRole('listitem', { name: 'Not available in this build' })).toHaveTextContent(/Case creation/);
  });

  it('shows an error state with retry when the data source fails', async () => {
    let fail = true;
    const repository: WorkspaceRepository = {
      ...syntheticRepository,
      listCases: (signal) => (fail ? Promise.reject(new Error('Case service unavailable')) : syntheticRepository.listCases(signal)),
    };
    await renderWithRepository('/workspace', repository);
    expect(screen.getAllByText('Case service unavailable').length).toBeGreaterThan(0);
    fail = false;
    await click(screen.getAllByRole('button', { name: 'Retry' })[0]!);
    expect(screen.getAllByText('Warehouse Access Investigation').length).toBeGreaterThan(0);
  });

  it('shows an empty state when there are no cases', async () => {
    await renderWithRepository('/workspace', { ...syntheticRepository, defaultCaseId: null, listCases: async () => [] });
    expect(screen.getByText('No cases yet')).toBeInTheDocument();
  });

  it('shows a loading state while cases load', async () => {
    await renderWithRepository('/workspace', { ...syntheticRepository, listCases: () => new Promise(() => {}) });
    expect(screen.getByText(/Loading cases/)).toBeInTheDocument();
  });
});

/* ------------------------------------------------------------- Case view */

describe('case view', () => {
  it('shows an honest empty state for summary-only cases', async () => {
    await renderAppAt('/workspace/cases/CASE-024/graph');
    expect(screen.getByRole('heading', { level: 1, name: 'Retail Card Skimming Series' })).toBeInTheDocument();
    expect(screen.getByText('Detailed records not available')).toBeInTheDocument();
  });

  it('shows not-found for an unknown case', async () => {
    await renderAppAt('/workspace/cases/CASE-999');
    expect(screen.getByText('Case not found')).toBeInTheDocument();
  });

  it('overview links each summary figure to its section', async () => {
    await renderAppAt('/workspace/cases/CASE-026');
    const summary = screen.getByRole('region', { name: 'Investigation summary' });
    expect(within(summary).getByText('Needs review').nextSibling).toHaveTextContent('04');
    // Visual previews are derived from the case records and link into the focused views.
    expect(screen.getByRole('link', { name: 'Person A, Person' })).toHaveAttribute('href', '/workspace/cases/CASE-026/graph?node=ENT-PA');
    expect(screen.getAllByRole('img', { name: /^Synthetic preview of EV-0/ })).toHaveLength(5);
    await click(within(screen.getByRole('region', { name: /Review queue/i })).getByText('Gate B timestamps disagree'));
    expect(window.location.search).toBe('?item=DX-01');
  });
});

/* ------------------------------------------------------------- Evidence */

describe('evidence repository', () => {
  it('filters, selects and inspects an artifact with integrity and provenance', async () => {
    await renderAppAt('/workspace/cases/CASE-026/evidence');
    // Gallery is the default view; every card carries a preview labelled synthetic or a neutral type glyph.
    const gallery = () => screen.getByRole('list', { name: 'Evidence gallery' });
    expect(within(gallery()).getAllByRole('listitem')).toHaveLength(5);
    expect(within(gallery()).getByRole('img', { name: 'Synthetic preview of EV-01' })).toBeInTheDocument();
    await click(screen.getByRole('button', { name: /^Flagged/ }));
    expect(within(gallery()).getAllByRole('listitem')).toHaveLength(2);
    await click(screen.getByRole('button', { name: /^All/ }));

    // The list view keeps the tabular comparison.
    await click(screen.getByRole('button', { name: 'List view' }));
    expect(screen.getAllByRole('row')).toHaveLength(6);
    await act(async () => {
      fireEvent.change(screen.getByRole('combobox', { name: /^Type/ }), { target: { value: 'UNCLASSIFIED' } });
    });
    expect(screen.getAllByRole('row')).toHaveLength(3);
    await act(async () => {
      fireEvent.change(screen.getByRole('combobox', { name: /^Type/ }), { target: { value: 'all' } });
    });

    await click(screen.getByRole('button', { name: /door_controller_export\.csv/ }));
    expect(window.location.search).toBe('?evidence=EV-02');
    const panel = screen.getByRole('complementary');
    expect(within(panel).getByRole('img', { name: 'Synthetic preview of EV-02' })).toBeInTheDocument();
    expect(within(panel).getByText('3b77e0a95c12d4f86b3e1a7c09d5f2e4a68b17c3e95d0f2a4c71b8e36d500c92')).toBeInTheDocument();
    expect(within(panel).getByText(/does not establish that the\s+artifact was authentic/)).toBeInTheDocument();
    expect(within(panel).getByText('Access-control log · no canonical EvidenceType')).toBeInTheDocument();
    expect(within(panel).getByRole('link', { name: /^Badge 4471 DEVICE/ })).toHaveAttribute('href', '/workspace/cases/CASE-026/graph?node=ENT-BADGE');

    await click(within(panel).getByRole('button', { name: /gateB_cam03/ }));
    expect(window.location.search).toBe('?evidence=EV-01');
    await click(within(screen.getByRole('complementary')).getByRole('button', { name: 'Close evidence detail' }));
    expect(screen.queryByRole('complementary')).not.toBeInTheDocument();
  });

  it('stages files locally with a SHA-256 preview and never submits', async () => {
    await renderAppAt('/workspace/cases/CASE-026/evidence');
    await click(screen.getByRole('button', { name: 'Add evidence' }));
    const input = screen.getByLabelText('Choose evidence files');
    await act(async () => {
      fireEvent.change(input, { target: { files: [new File(['abc'], 'note.txt', { type: 'text/plain' })] } });
    });
    expect(await screen.findByText('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad', { exact: false })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Submit for ingestion' })).toBeDisabled();
    await click(screen.getByRole('button', { name: 'Remove note.txt' }));
    expect(screen.queryByText('note.txt')).not.toBeInTheDocument();
  });
});

/* ---------------------------------------------------------------- Graph */

describe('knowledge graph', () => {
  it('selects nodes by keyboard and shows unavailable confidence as "Not provided"', async () => {
    await renderAppAt('/workspace/cases/CASE-026/graph');
    const node = screen.getByRole('button', { name: 'Person A, Person' });
    await act(async () => {
      fireEvent.keyDown(node, { key: 'Enter' });
    });
    expect(window.location.search).toBe('?node=ENT-PA');
    const inspector = screen.getByRole('complementary', { name: 'Inspector' });
    expect(within(inspector).getByText('Confidence').nextSibling).toHaveTextContent('Not provided');
    expect(within(inspector).getByText('Connected records · 6')).toBeInTheDocument();
  });

  it('inspects a relationship with relation_type, is_observed and weight', async () => {
    await renderAppAt('/workspace/cases/CASE-026/graph?node=ENT-PA');
    const inspector = screen.getByRole('complementary', { name: 'Inspector' });
    await click(within(inspector).getByRole('button', { name: 'Inspect relationship likely user of' }));
    expect(window.location.search).toBe('?edge=R2');
    const updated = screen.getByRole('complementary', { name: 'Inspector' });
    expect(within(updated).getByText('OWNS')).toBeInTheDocument();
    expect(within(updated).getByText('is_observed').nextSibling).toHaveTextContent('false');
    expect(within(updated).getByText('weight').nextSibling).toHaveTextContent('Not provided');
  });

  it('filters node types, searches, zooms and traces a path', async () => {
    await renderAppAt('/workspace/cases/CASE-026/graph?node=ENT-ACC');
    expect(screen.queryByRole('button', { name: /EV-01, Evidence artifact/ })).not.toBeInTheDocument();
    await click(screen.getByRole('button', { name: 'Evidence artifact' }));
    expect(screen.getByRole('button', { name: /EV-01, Evidence artifact/ })).toBeInTheDocument();

    await act(async () => {
      fireEvent.change(screen.getByRole('searchbox', { name: 'Search nodes' }), { target: { value: 'gate' } });
    });
    expect(screen.getByText(/\d+ match/)).toBeInTheDocument();

    const svg = screen.getByRole('group', { name: /Knowledge graph for CASE-026/ });
    const before = svg.getAttribute('viewBox');
    await click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(svg.getAttribute('viewBox')).not.toBe(before);
    await click(screen.getByRole('button', { name: 'Reset view' }));
    expect(svg.getAttribute('viewBox')).toBe('0 0 1000 640');

    await act(async () => {
      fireEvent.change(screen.getByRole('combobox', { name: /^To/ }), { target: { value: 'ENT-LOC' } });
    });
    expect(screen.getByText('Path highlighted on the canvas.')).toBeInTheDocument();
  });

  it('draws contradiction links distinctly and lists them in the legend', async () => {
    await renderAppAt('/workspace/cases/CASE-026/graph');
    expect(screen.getByText('Contradicts (needs review)')).toBeInTheDocument();
    expect(screen.getByText(/≠/)).toBeInTheDocument();
  });
});

/* -------------------------------------------------------------- Timeline */

describe('event reconstruction', () => {
  it('shows source time zone, recorded value, precision and uncertainty for the selected event', async () => {
    await renderAppAt('/workspace/cases/CASE-026/timeline?event=EVT-3');
    const detail = screen.getByRole('complementary');
    expect(within(detail).getByText('14/03/2026 22:41:07 (camera overlay)')).toBeInTheDocument();
    expect(within(detail).getByText(/Assumed IST — camera time-zone setting not confirmed/)).toBeInTheDocument();
    expect(within(detail).getByText('CONCURRENT_UNCERTAIN')).toBeInTheDocument();
    expect(within(detail).getByText(/±3 min investigator estimate/)).toBeInTheDocument();
  });

  it('keeps minute-precision events at minute precision and makes conflicts visible', async () => {
    await renderAppAt('/workspace/cases/CASE-026/timeline');
    expect(screen.getAllByText('21:58').length).toBeGreaterThan(0);
    expect(screen.queryByText('21:58:00')).not.toBeInTheDocument();
    expect(screen.getByText(/Δ \+2 min 45 s/)).toBeInTheDocument();
    expect(screen.getByText('3 events cannot be strictly ordered')).toBeInTheDocument();
  });

  it('filters by source and event type and selects from the chronology', async () => {
    await renderAppAt('/workspace/cases/CASE-026/timeline');
    await click(screen.getByRole('button', { name: 'EV-04' }));
    expect(screen.queryByRole('button', { name: 'Card payment at fuel station' })).not.toBeInTheDocument();
    await act(async () => {
      fireEvent.change(screen.getByRole('combobox', { name: /^Type/ }), { target: { value: 'Physical access' } });
    });
    const table = screen.getByRole('table');
    expect(within(table).getAllByRole('row')).toHaveLength(3);
    await click(within(table).getByRole('button', { name: 'Badge 4471 granted entry' }));
    expect(window.location.search).toBe('?event=EVT-4');
    await click(screen.getByRole('button', { name: '22:30 – 22:50' }));
    expect(screen.getByRole('button', { name: '22:30 – 22:50' })).toHaveAttribute('aria-pressed', 'true');
  });
});

/* ------------------------------------------------- Review / spatial / etc. */

describe('contradiction review', () => {
  it('filters by category and shows records side by side with limitations', async () => {
    await renderAppAt('/workspace/cases/CASE-026/contradictions');
    expect(screen.getByText(/not yet exposed\s+through an API/)).toBeInTheDocument();
    const detail = screen.getByRole('complementary');
    expect(within(detail).getByText('Source A · EV-01')).toBeInTheDocument();
    expect(within(detail).getByText('Source B · EV-02')).toBeInTheDocument();
    expect(within(detail).getByText(/not evidence of wrongdoing/)).toBeInTheDocument();
    // Both recorded clock values are plotted on one axis, exactly as recorded.
    expect(within(detail).getByRole('img', { name: /Source A at 22:41:07 IST and source B at 22:43:52 IST, 2 min 45 s apart as recorded/ })).toBeInTheDocument();

    await click(within(screen.getByRole('group', { name: 'Filter review items by category' })).getByRole('button', { name: /^Missing information/ }));
    const queue = screen.getByRole('region', { name: /Review queue/ });
    expect(within(queue).getAllByRole('listitem')).toHaveLength(1);
    expect(within(screen.getByRole('complementary')).getByRole('heading', { name: 'No interior footage after 22:42' })).toBeInTheDocument();
    await click(screen.getByRole('button', { name: /No interior footage after 22:42/ }));
    expect(window.location.search).toBe('?item=DX-03');
    expect(within(screen.getByRole('complementary')).getByText(/Absence of footage is not evidence of absence/)).toBeInTheDocument();
  });
});

describe('spatial reconstruction', () => {
  it('selects a place and lists events without location', async () => {
    await renderAppAt('/workspace/cases/CASE-026/spatial');
    expect(screen.getByText(/schematic · not to scale · not geographic/)).toBeInTheDocument();
    expect(screen.getByText('Message sent from device D-01')).toBeInTheDocument();
    await act(async () => {
      fireEvent.keyDown(screen.getByRole('button', { name: /Access point AP-WH7/ }), { key: 'Enter' });
    });
    const panel = screen.getByRole('complementary');
    expect(within(panel).getByText('Area (radio coverage), not a point')).toBeInTheDocument();
    expect(within(panel).getByText(/D-01 joins warehouse access point/)).toBeInTheDocument();
  });
});

describe('evidence correlation', () => {
  it('highlights the artifacts behind a selected relationship', async () => {
    await renderAppAt('/workspace/cases/CASE-026/correlation');
    await click(screen.getByRole('button', { name: /^inferred/i }));
    await click(screen.getByRole('button', { name: /Person A — likely user of → Device D-01/ }));
    expect(window.location.search).toBe('?rel=R2');
    expect(screen.getByText(/Highlighting the artifacts that support/)).toBeInTheDocument();
    expect(screen.getAllByText(/weight: not provided/).length).toBeGreaterThan(0);
  });
});

describe('reports', () => {
  it('assembles a preview, toggles sections, prints, and keeps export disabled', async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => {});
    await renderAppAt('/workspace/cases/CASE-026/reports');
    const preview = screen.getByRole('article', { name: 'Report preview' });
    expect(within(preview).getByText('Evidence inventory')).toBeInTheDocument();
    await click(screen.getByRole('button', { name: 'Evidence inventory' }));
    expect(within(preview).queryByText('Evidence inventory')).not.toBeInTheDocument();
    await click(screen.getByRole('button', { name: 'Print preview' }));
    expect(print).toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Export PDF / JSON' })).toBeDisabled();
  });
});

describe('settings and help', () => {
  it('shows the real runtime configuration', async () => {
    await renderAppAt('/workspace/settings');
    expect(screen.getByText(getApiBaseUrl())).toBeInTheDocument();
    expect(screen.getByText(/GET \/health every 15s/)).toBeInTheDocument();
    expect(screen.getByText('Synthetic fixture (no case API yet)')).toBeInTheDocument();
  });

  it('explains provenance, review categories and precision', async () => {
    await renderAppAt('/workspace/help');
    expect(screen.getByText(/never proof of identity/)).toBeInTheDocument();
    expect(screen.getByText('EXACT_SECOND')).toBeInTheDocument();
  });
});

/* ------------------------------------------------------ Logo → homepage */

describe('workspace logo navigation', () => {
  const logo = (scope: HTMLElement = sidebar()): HTMLElement => within(scope).getByRole('link', { name: 'Go to YUKTI homepage' });

  it.each([
    '/workspace',
    '/workspace/cases/CASE-026',
    '/workspace/cases/CASE-026/evidence?evidence=EV-01',
    '/workspace/cases/CASE-026/graph?node=ENT-PA',
    '/workspace/settings',
  ])('returns to the public homepage from %s without a full reload', async (path) => {
    await renderAppAt(path);
    const link = logo();
    expect(link).toHaveAttribute('href', '/');
    expect(link.tagName).toBe('A'); // natively focusable and keyboard-activatable

    const pushState = vi.spyOn(window.history, 'pushState');
    const event = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 });
    await act(async () => {
      link.dispatchEvent(event);
    });

    expect(event.defaultPrevented).toBe(true); // client-side navigation, no document reload
    expect(pushState).toHaveBeenCalledWith(null, '', '/');
    expect(window.location.pathname).toBe('/');
    expect(window.location.search).toBe('');
    expect(screen.getByRole('heading', { level: 1, name: /every fragment tells a story/i })).toBeInTheDocument();
    expect(screen.queryByRole('navigation', { name: 'Workspace' })).not.toBeInTheDocument();
  });

  it('supports browser back and forward after returning home', async () => {
    await renderAppAt('/workspace/cases/CASE-026/timeline');
    await click(logo());
    expect(window.location.pathname).toBe('/');

    await act(async () => {
      window.history.back();
      await new Promise((resolve) => window.addEventListener('popstate', resolve, { once: true }));
    });
    expect(window.location.pathname).toBe('/workspace/cases/CASE-026/timeline');
    expect(screen.getByRole('heading', { level: 1, name: 'Warehouse Access Investigation' })).toBeInTheDocument();

    await act(async () => {
      window.history.forward();
      await new Promise((resolve) => window.addEventListener('popstate', resolve, { once: true }));
    });
    expect(window.location.pathname).toBe('/');
  });

  it('works when the sidebar is collapsed and does not toggle the collapse state', async () => {
    await renderAppAt('/workspace');
    await click(screen.getByRole('button', { name: 'Collapse sidebar' }));
    expect(window.localStorage.getItem('yukti.workspace.sidebarCollapsed')).toBe('1');
    await click(logo());
    expect(window.location.pathname).toBe('/');
    expect(window.localStorage.getItem('yukti.workspace.sidebarCollapsed')).toBe('1');
  });

  it('works from the mobile drawer and closes it', async () => {
    await renderAppAt('/workspace/cases/CASE-026/evidence');
    await click(screen.getByRole('button', { name: 'Open navigation' }));
    await click(logo(screen.getByRole('dialog', { name: 'Workspace navigation' })));
    expect(window.location.pathname).toBe('/');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(document.body.style.overflow).toBe('');
  });

  it('keeps the workspace overview reachable from the sidebar', async () => {
    await renderAppAt('/workspace/cases/CASE-026');
    await click(within(sidebar()).getByRole('link', { name: 'Investigations' }));
    expect(window.location.pathname).toBe('/workspace');
  });
});

describe('sidebar preference migration (SUTRAM → YUKTI rename)', () => {
  it('migrates a saved legacy preference once and keeps the user\'s choice', async () => {
    window.localStorage.setItem('sutram.workspace.sidebarCollapsed', '1');
    await renderAppAt('/workspace');
    expect(screen.getByRole('button', { name: 'Expand sidebar' })).toBeInTheDocument();
    expect(window.localStorage.getItem('yukti.workspace.sidebarCollapsed')).toBe('1');
    expect(window.localStorage.getItem('sutram.workspace.sidebarCollapsed')).toBeNull();
  });

  it('prefers the new key when both exist', async () => {
    window.localStorage.setItem('yukti.workspace.sidebarCollapsed', '0');
    window.localStorage.setItem('sutram.workspace.sidebarCollapsed', '1');
    await renderAppAt('/workspace');
    expect(screen.getByRole('button', { name: 'Collapse sidebar' })).toBeInTheDocument();
  });

  it('settings reset clears both keys', async () => {
    window.localStorage.setItem('yukti.workspace.sidebarCollapsed', '1');
    window.localStorage.setItem('sutram.workspace.sidebarCollapsed', '1');
    await renderAppAt('/workspace/settings');
    await click(screen.getByRole('button', { name: 'Reset sidebar preference' }));
    expect(window.localStorage.getItem('yukti.workspace.sidebarCollapsed')).toBeNull();
    expect(window.localStorage.getItem('sutram.workspace.sidebarCollapsed')).toBeNull();
  });
});
