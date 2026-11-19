import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, within } from '@testing-library/react';
import { LandingPage } from './LandingPage';
import { NAV_ITEMS, PROJECT_REPOSITORY_URL } from '../features/landing/data/siteContent';

function stubHealth(ok: boolean) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () =>
      ok
        ? new Response(JSON.stringify({ status: 'ok', project: 'sutram-digital-forensics' }))
        : Promise.reject(new TypeError('Failed to fetch')),
    ),
  );
}

async function renderLanding() {
  await act(async () => {
    render(<LandingPage />);
  });
}

beforeEach(() => stubHealth(true));
afterEach(() => vi.unstubAllGlobals());

describe('LandingPage structure', () => {
  it('renders every section of the product story in order', async () => {
    await renderLanding();
    // Accessible names must keep word boundaries between the two headline lines.
    const headings = screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent);
    expect(headings).toEqual([
      'Six kinds of evidence. One board.',
      'Different sources. Connected evidence.',
      'Put every record back in its place.',
      'Reconstruct the sequence. Understand the connections.',
      'Not every connection is a confirmation.',
      'Every insight has a source.',
      'From the board to the case file.',
      'Built to be checked, not just believed.',
      'Bring every piece of evidence into the bigger picture.',
      'About YUKTI',
    ]);
  });

  it('has a single h1 that states what YUKTI does', async () => {
    await renderLanding();
    const h1 = screen.getAllByRole('heading', { level: 1 });
    expect(h1).toHaveLength(1);
    expect(h1[0]).toHaveTextContent(/YUKTI\s*Every fragment tells a story\. Every connection changes the investigation\./i);
  });

  it('has no dead in-page links: every #anchor resolves to an element', async () => {
    const { container } = render(<LandingPage />);
    await act(async () => {});
    const hashLinks = [...container.querySelectorAll<HTMLAnchorElement>('a[href^="#"]')];
    expect(hashLinks.length).toBeGreaterThan(20);
    for (const link of hashLinks) {
      const id = link.getAttribute('href')!.slice(1);
      expect(document.getElementById(id), `missing target for ${link.textContent} → #${id}`).not.toBeNull();
    }
  });

  it('exposes the five primary navigation items with section targets', async () => {
    await renderLanding();
    const nav = screen.getByRole('navigation', { name: 'Primary' });
    for (const item of NAV_ITEMS) {
      expect(within(nav).getByRole('link', { name: item.label })).toHaveAttribute('href', item.href);
    }
  });

  it('routes workspace CTAs to /workspace and the repository opens safely in a new tab', async () => {
    await renderLanding();
    for (const link of screen.getAllByRole('link', { name: /open workspace|explore yukti|investigator workspace/i })) {
      expect(link).toHaveAttribute('href', '/workspace');
    }
    const repo = screen.getByRole('link', { name: /repository/i });
    expect(repo).toHaveAttribute('href', PROJECT_REPOSITORY_URL);
    expect(repo).toHaveAttribute('target', '_blank');
    expect(repo).toHaveAttribute('rel', 'noopener noreferrer');
  });

  it('labels demonstration data as synthetic', async () => {
    await renderLanding();
    expect(screen.getAllByText('Synthetic demonstration data').length).toBeGreaterThan(0);
    expect(screen.getByText(/records are fictional and were not processed from uploaded evidence/i)).toBeInTheDocument();
  });

  it('labels deliverable status honestly and never claims planned work is implemented', async () => {
    await renderLanding();
    const section = document.getElementById('build-status')!;
    expect(within(section).getAllByText('Planned')).toHaveLength(2);
    expect(within(section).getAllByText('In development')).toHaveLength(2);
    expect(within(section).getAllByText('Prototype')).toHaveLength(1);
    expect(within(section).queryByText('Implemented')).not.toBeInTheDocument();
  });

  it('uses forensically careful wording about hashing and investigator review', async () => {
    await renderLanding();
    expect(screen.getByText(/a digest alone does not establish that the original source is authentic/i)).toBeInTheDocument();
    expect(screen.getByText(/it does not determine identity or guilt/i)).toBeInTheDocument();
  });
});

describe('LandingPage footer API status (existing health monitor)', () => {
  it('shows a connected backend', async () => {
    await renderLanding();
    expect(await screen.findByText('Backend API connected')).toBeInTheDocument();
  });

  it('shows an offline backend', async () => {
    stubHealth(false);
    await renderLanding();
    expect(await screen.findByText('Backend API offline')).toBeInTheDocument();
  });
});
