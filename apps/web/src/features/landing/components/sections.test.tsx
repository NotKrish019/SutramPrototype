import { describe, expect, it, vi, afterEach } from 'vitest';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { ContradictionSection } from './ContradictionSection';
import { CorrelationBoard } from './CorrelationBoard';
import { EvidenceSourcesBoard } from './EvidenceSourcesBoard';
import { ProvenanceChain } from './ProvenanceChain';
import { ReconstructionSection } from './ReconstructionSection';
import { SiteNavbar } from './SiteHeader';
import { ThreadCanvas } from '../board/ThreadCanvas';

afterEach(() => vi.restoreAllMocks());

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
    fireEvent.click(panel.querySelector('a[href="#reconstruction"]')!);
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

describe('EvidenceSourcesBoard', () => {
  it('presents the six evidence categories as labelled artifacts', () => {
    render(<EvidenceSourcesBoard />);
    const section = document.getElementById('capabilities')!;
    expect(within(section).getAllByRole('figure')).toHaveLength(6);
    for (const title of ['CCTV & video', 'Mobile devices', 'Fingerprint & forensic records', 'Communication records', 'Financial & transactions', 'Location & network']) {
      expect(within(section).getByRole('figure', { name: title })).toBeInTheDocument();
    }
    expect(within(section).getByText(/synthetic demonstration artifacts/i)).toBeInTheDocument();
  });
});

describe('CorrelationBoard', () => {
  it('describes a selected record’s connections and keeps inferred links labelled', () => {
    render(<CorrelationBoard />);
    expect(screen.getByText('Hover or select a record to see what it connects to.')).toBeInTheDocument();
    const person = screen.getByRole('button', { name: /Person A/ });
    fireEvent.click(person);
    expect(person).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText(/connections:.*likely user of · inferred/i)).toBeInTheDocument();
  });

  it('traces the related-entity path and flags its inferred link for review', () => {
    render(<CorrelationBoard />);
    const trace = screen.getByRole('button', { name: /trace related path/i });
    fireEvent.click(trace);
    expect(screen.getByRole('button', { name: /clear path/i })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText(/one link is inferred and requires review/i)).toBeInTheDocument();
  });
});

describe('ReconstructionSection', () => {
  it('selecting a sensor in the 3D model or the dossier list selects the same sensor', () => {
    render(<ReconstructionSection />);
    // The dossier list uses <button>; the 3D model exposes its markers as SVG role="button" groups.
    const listButton = (name: RegExp) => screen.getAllByRole('button', { name }).find((b) => b.tagName === 'BUTTON')!;
    const marker = (name: RegExp) => screen.getAllByRole('button', { name }).find((b) => b.tagName.toLowerCase() === 'g')!;

    expect(listButton(/CAM-03 · Gate B camera/)).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(listButton(/ACS-B · Gate B controller/));
    expect(listButton(/ACS-B · Gate B controller/)).toHaveAttribute('aria-pressed', 'true');
    expect(listButton(/CAM-03 · Gate B camera/)).toHaveAttribute('aria-pressed', 'false');

    fireEvent.click(marker(/AP-WH7/));
    expect(listButton(/AP-WH7/)).toHaveAttribute('aria-pressed', 'true');
  });

  it('toggles the reconstructed route', () => {
    render(<ReconstructionSection />);
    const route = screen.getByRole('button', { name: /reconstructed route/i });
    const before = route.getAttribute('aria-pressed');
    fireEvent.click(route);
    expect(route.getAttribute('aria-pressed')).not.toBe(before);
  });
});

describe('ContradictionSection', () => {
  it('describes both clocks and never treats a discrepancy as wrongdoing', () => {
    render(<ContradictionSection />);
    expect(screen.getByRole('img', { name: /camera clock.*controller clock.*requires verification/i })).toBeInTheDocument();
    expect(screen.getByText(/not evidence of wrongdoing/i)).toBeInTheDocument();
    expect(screen.getByText(/possible explanations · not assessed/i)).toBeInTheDocument();
  });
});

describe('ProvenanceChain', () => {
  it('traces a focused step back to its source and marks the lead as inferred', () => {
    render(<ProvenanceChain />);
    const steps = [...document.querySelectorAll<HTMLElement>('#provenance ol > li [tabindex="0"]')];
    expect(steps).toHaveLength(4);
    const status = document.querySelector('#provenance [aria-live="polite"]')!;
    const idle = status.textContent;
    fireEvent.focus(steps[3]!);
    expect(status.textContent).not.toBe(idle);
    expect(within(steps[3]!).getByText(/inferred/i)).toBeInTheDocument();
  });
});

describe('ThreadCanvas', () => {
  it('draws zero-size anchors but skips anchors hidden at this breakpoint', () => {
    const rect = (w: number, h: number, x = 0, y = 0) => ({ width: w, height: h, left: x, top: y, right: x + w, bottom: y + h, x, y, toJSON: () => ({}) }) as DOMRect;
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      const id = this.getAttribute('data-pin-id');
      if (id === 'edge' || id === 'hidden') return rect(0, 0, id === 'edge' ? 0 : 0, 40);
      if (id) return rect(10, 10, id === 'a' ? 20 : 200, 20);
      return rect(400, 300);
    });
    // jsdom has no checkVisibility; emulate a browser where only `hidden` is display:none.
    Object.defineProperty(Element.prototype, 'checkVisibility', {
      configurable: true,
      value(this: Element) {
        return this.getAttribute('data-pin-id') !== 'hidden';
      },
    });
    try {
      const { container } = render(
        <div>
          <span data-pin-id="a" />
          <span data-pin-id="b" />
          <span data-pin-id="edge" />
          <span data-pin-id="hidden" />
          <ThreadCanvas
            drawn
            pulse={false}
            links={[
              { id: 'visible', from: 'a', to: 'b' },
              { id: 'edge-tie', from: 'edge', to: 'b' },
              { id: 'to-hidden', from: 'a', to: 'hidden' },
            ]}
          />
        </div>,
      );
      expect(container.querySelectorAll('path.thread')).toHaveLength(2);
    } finally {
      delete (Element.prototype as { checkVisibility?: unknown }).checkVisibility;
    }
  });
});
