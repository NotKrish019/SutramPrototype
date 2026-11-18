import { describe, expect, it, vi, afterEach } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { CAPABILITIES, WORKFLOW_STEPS } from '../data/siteContent';
import { EvidenceNetworkVisual } from './EvidenceNetworkVisual';
import { PlatformSection } from './PlatformSection';
import { ProvenanceSection } from './ProvenanceSection';
import { SiteNavbar } from './SiteHeader';
import { WorkflowSection } from './WorkflowSection';

afterEach(() => vi.restoreAllMocks());

const lastTab = (): HTMLElement => {
  const tabs = screen.getAllByRole('tab');
  return tabs[tabs.length - 1]!;
};

describe('PlatformSection capability tabs', () => {
  it('shows capability A first and switches on click', () => {
    render(<PlatformSection />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(CAPABILITIES.length);
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true');

    fireEvent.click(screen.getByRole('tab', { name: /Contradiction/ }));
    expect(screen.getByRole('tab', { name: /Contradiction/ })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tabpanel')).toHaveTextContent('Timestamp variance');
  });

  it('supports arrow, Home and End keys with roving tabindex', () => {
    render(<PlatformSection />);
    const tabs = screen.getAllByRole('tab');
    fireEvent.keyDown(tabs[0]!, { key: 'ArrowDown' });
    expect(screen.getAllByRole('tab')[1]).toHaveAttribute('aria-selected', 'true');
    expect(screen.getAllByRole('tab')[1]).toHaveAttribute('tabindex', '0');
    expect(screen.getAllByRole('tab')[0]).toHaveAttribute('tabindex', '-1');

    fireEvent.keyDown(screen.getAllByRole('tab')[1]!, { key: 'End' });
    expect(lastTab()).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(lastTab(), { key: 'ArrowRight' });
    expect(screen.getAllByRole('tab')[0]).toHaveAttribute('aria-selected', 'true'); // wraps
    fireEvent.keyDown(screen.getAllByRole('tab')[0]!, { key: 'ArrowUp' });
    expect(lastTab()).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(lastTab(), { key: 'Home' });
    expect(screen.getAllByRole('tab')[0]).toHaveAttribute('aria-selected', 'true');
  });

  it('explainability visual distinguishes inferred from observed and disclaims identity proof', () => {
    render(<PlatformSection />);
    fireEvent.click(screen.getByRole('tab', { name: /Explainable/ }));
    expect(screen.getByText(/not proof of identity/i)).toBeInTheDocument();
  });
});

describe('WorkflowSection stepper', () => {
  it('starts at Ingest and advances through stages', () => {
    render(<WorkflowSection />);
    expect(screen.getByText(`Stage 01 / 0${WORKFLOW_STEPS.length}`)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /next: extract/i }));
    expect(screen.getByText('Stage 02 / 04')).toBeInTheDocument();
    expect(screen.getByText(/4 values extracted/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /04 Reconstruct/ }));
    expect(screen.getByRole('button', { name: /04 Reconstruct/ })).toHaveAttribute('aria-current', 'step');
    expect(screen.getAllByText('order uncertain').length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole('button', { name: /restart from ingest/i }));
    expect(screen.getByText('Stage 01 / 04')).toBeInTheDocument();
  });
});

describe('ProvenanceSection trace chain', () => {
  it('traces a focused step back to its artifact', () => {
    render(<ProvenanceSection />);
    expect(screen.getByText(/hover or focus a step/i)).toBeInTheDocument();
    const lead = screen.getByText(/Verify whether Person A entered Gate B/).closest('[tabindex]')!;
    fireEvent.focus(lead);
    expect(screen.getByText(/tracing investigative lead back through 3 supporting steps to EV-02/i)).toBeInTheDocument();
    fireEvent.blur(lead);
    expect(screen.getByText(/hover or focus a step/i)).toBeInTheDocument();
  });
});

describe('EvidenceNetworkVisual', () => {
  it('highlights a source on hover/focus (desktop) and toggles via chips (mobile)', () => {
    render(<EvidenceNetworkVisual />);
    const [desktopSource] = screen.getAllByRole('button', { name: 'Trace Financial evidence' });
    fireEvent.focus(desktopSource!);
    expect(desktopSource).toHaveAttribute('aria-pressed', 'true');
    fireEvent.blur(desktopSource!);
    expect(desktopSource).toHaveAttribute('aria-pressed', 'false');

    const chip = screen.getByRole('button', { name: 'CCTV / video' });
    fireEvent.click(chip);
    expect(chip).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(chip);
    expect(chip).toHaveAttribute('aria-pressed', 'false');
  });

  it('labels the visual as synthetic and describes it for assistive tech', () => {
    render(<EvidenceNetworkVisual />);
    expect(screen.getByText('Synthetic data')).toBeInTheDocument();
    expect(screen.getAllByRole('img', { name: /six evidence source types feed a correlation layer/i }).length).toBe(2);
  });
});

describe('SiteNavbar mobile menu', () => {
  it('opens, closes on Escape (restoring focus) and closes after choosing a link', () => {
    render(<SiteNavbar />);
    const toggle = screen.getByRole('button', { name: 'Open menu' });
    const panel = document.getElementById(toggle.getAttribute('aria-controls')!)!;
    expect(panel).not.toBeVisible();

    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(toggle).toHaveAccessibleName('Close menu');
    expect(panel).toBeVisible();

    act(() => {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(panel).not.toBeVisible();
    expect(document.activeElement).toBe(toggle);

    fireEvent.click(toggle);
    fireEvent.click(panel.querySelector('a[href="#workflow"]')!);
    expect(panel).not.toBeVisible();
  });

  it('closes when clicking outside the header', () => {
    render(
      <div>
        <SiteNavbar />
        <p>outside</p>
      </div>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Open menu' }));
    act(() => {
      screen.getByText('outside').dispatchEvent(new Event('pointerdown', { bubbles: true }));
    });
    expect(screen.getByRole('button', { name: 'Open menu' })).toHaveAttribute('aria-expanded', 'false');
  });
});

describe('visual-first refinements', () => {
  it('3D hero stack traces a source through entities and events', async () => {
    const { HeroStack3D } = await import('./HeroStack3D');
    render(<HeroStack3D />);
    const finance = screen.getByRole('button', { name: 'Trace Finance evidence' });
    fireEvent.focus(finance);
    expect(finance).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText(/Tracing Finance → 2 entities · 1 events/)).toBeInTheDocument();
    fireEvent.blur(finance);
    expect(screen.getByText(/a source to trace it/)).toBeInTheDocument();
    expect(screen.getByText(/synthetic data/i)).toBeInTheDocument();
  });

  it('sources list drives the network visual and the artifact preview', async () => {
    const { EvidenceSourcesSection } = await import('./EvidenceSourcesSection');
    render(<EvidenceSourcesSection />);
    const item = screen.getByRole('button', { name: /Financial Records/ });
    fireEvent.mouseEnter(item);
    expect(item).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getAllByRole('button', { name: 'Trace Financial evidence' })[0]).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText('Artifact preview · Financial Records')).toBeInTheDocument();
  });

  it('graph tab shows a detail card with provenance for the selected node', () => {
    render(<PlatformSection />);
    fireEvent.click(screen.getByRole('tab', { name: /Knowledge Graph/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Device D-01, Device' }));
    const card = screen.getByText('associated at Warehouse 7 · Gate B').closest('div')!;
    expect(card).toHaveTextContent('Device D-01');
    expect(card).toHaveTextContent('Inferred');
  });

  it('reconstruction tab links the real swimlanes to the site plan', () => {
    render(<PlatformSection />);
    fireEvent.click(screen.getByRole('tab', { name: /Reconstruction/ }));
    fireEvent.click(screen.getByRole('button', { name: /D-01 joins warehouse access point/ }));
    expect(screen.getByRole('button', { name: /Access point AP-WH7/ })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: /Gate B, Gate/ })).toHaveAttribute('aria-pressed', 'false');
  });

  it('contradiction tab annotates the gap on a true-to-scale axis', () => {
    render(<PlatformSection />);
    fireEvent.click(screen.getByRole('tab', { name: /Contradiction/ }));
    expect(screen.getByRole('img', { name: /22:41:07 versus 22:43:52: \+2 min 45 s, clock synchronisation requires verification/ })).toBeInTheDocument();
  });
});
