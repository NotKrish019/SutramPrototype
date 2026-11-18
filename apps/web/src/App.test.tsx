import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import App from './App';

describe('App', () => {
  beforeEach(() => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({ status: 'ok', project: 'sutram-digital-forensics' }),
            { status: 200, headers: { 'Content-Type': 'application/json' } },
          ),
        ),
      ),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders project name and subtitle accurately', async () => {
    await act(async () => {
      render(<App />);
    });

    const headings = screen.getAllByText(/SUTRAM Digital Forensics/i);
    expect(headings.length).toBeGreaterThan(0);

    const subtitles = screen.getAllByText(
      /Digital Evidence Correlation and Crime Reconstruction Platform/i
    );
    expect(subtitles.length).toBeGreaterThan(0);
  });
});
