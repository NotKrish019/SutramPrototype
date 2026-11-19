import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { DEMO_CASE } from '../../../../fixtures/synthetic/case026';
import { ProductPreview } from './ProductPreview';
import { countReviewItems, deriveSelection } from './selection';

const provenanceHeading = () => document.querySelector('#platform [aria-live="polite"]')!;

describe('deriveSelection', () => {
  it('resolves an evidence item to its events and entities', () => {
    const d = deriveSelection({ kind: 'evidence', id: 'EV-01' });
    expect([...d.evidenceIds]).toEqual(['EV-01']);
    expect([...d.eventIds]).toEqual(['EVT-3']);
    expect([...d.entityIds].sort()).toEqual(['ENT-LOC', 'ENT-VEH']);
    expect(d.selectedEventId).toBeNull();
  });

  it('resolves an event to its source evidence', () => {
    const d = deriveSelection({ kind: 'event', id: 'EVT-4' });
    expect(d.selectedEventId).toBe('EVT-4');
    expect([...d.evidenceIds]).toEqual(['EV-02']);
  });

  it('resolves an entity to its neighbours and supporting artifacts', () => {
    const d = deriveSelection({ kind: 'entity', id: 'ENT-DEV' });
    expect(d.selectedEntityId).toBe('ENT-DEV');
    expect([...d.entityIds].sort()).toEqual(['ENT-DEV', 'ENT-LOC', 'ENT-PA']);
    expect([...d.evidenceIds].sort()).toEqual(['EV-03', 'EV-05']);
  });

  it('returns empty sets for unknown ids', () => {
    const d = deriveSelection({ kind: 'evidence', id: 'missing' });
    expect(d.evidenceIds.size + d.eventIds.size + d.entityIds.size).toBe(0);
  });

  it('counts items that still need review', () => {
    expect(countReviewItems()).toBe(DEMO_CASE.evidence.filter((e) => e.review !== 'verified').length);
  });
});

describe('ProductPreview workbench', () => {
  it('shows the case overview with counts derived from the synthetic case', () => {
    render(<ProductPreview />);
    expect(screen.getByText('Warehouse Access Investigation')).toBeInTheDocument();
    expect(screen.getByText('Synthetic demonstration data')).toBeInTheDocument();
    const overview = screen.getByText('Needs review').closest('dl')!;
    expect(within(overview).getByText('Evidence').nextSibling).toHaveTextContent('05');
    expect(within(overview).getByText('Needs review').nextSibling).toHaveTextContent(
      String(countReviewItems()).padStart(2, '0'),
    );
  });

  it('starts with the flagged CCTV artifact selected and shows its provenance', () => {
    render(<ProductPreview />);
    expect(provenanceHeading()).toHaveTextContent('EV-01 · CCTV · Gate B camera 03');
    expect(screen.getByRole('button', { name: /CCTV · Gate B camera 03/ })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText(/gate opens at 22:41:07 \(camera clock\)/i)).toBeInTheDocument();
  });

  it('updates the provenance panel when another artifact is selected', () => {
    render(<ProductPreview />);
    fireEvent.click(screen.getByRole('button', { name: /Transaction record/ }));
    expect(provenanceHeading()).toHaveTextContent('EV-04 · Transaction record');
    expect(screen.getByText('card_statement_mar2026.pdf', { selector: 'p' })).toBeInTheDocument();
    // Transaction record is supplied, not observed.
    expect(screen.getByText('Provided by a party or record holder; not independently observed.')).toBeInTheDocument();
  });

  it('selecting a graph entity shows relationships with provenance and supporting artifacts', () => {
    render(<ProductPreview />);
    fireEvent.click(screen.getByRole('button', { name: 'Person A, Person' }));
    expect(provenanceHeading()).toHaveTextContent('Person A · person');
    const panel = provenanceHeading().parentElement!;
    expect(within(panel).getByText('likely user of')).toBeInTheDocument();
    // Person A's links to D-01 and V-1 are both correlation-derived.
    expect(within(panel).getAllByText('Inferred')).toHaveLength(2);
    expect(within(panel).getAllByText('Supplied')).toHaveLength(2);

    // Drill back to a supporting artifact.
    fireEvent.click(screen.getByRole('button', { name: /EV-03 · device_D01_chat_export.json/ }));
    expect(provenanceHeading()).toHaveTextContent('EV-03 · Communication export');
  });

  it('graph entities are keyboard operable', () => {
    render(<ProductPreview />);
    const node = screen.getByRole('button', { name: 'Vehicle V-1, Vehicle' });
    fireEvent.keyDown(node, { key: 'Enter' });
    expect(provenanceHeading()).toHaveTextContent('Vehicle V-1 · vehicle');
  });

  it('switches between graph and timeline views', () => {
    render(<ProductPreview />);
    const timelineTab = screen.getByRole('tab', { name: /event timeline/i });
    fireEvent.click(timelineTab);
    expect(timelineTab).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('list', { name: /event timeline/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /D-01 joins warehouse access point/ }));
    expect(provenanceHeading()).toHaveTextContent('22:44:10 · D-01 joins warehouse access point');
  });

  it('"Show in timeline" opens the timeline at the flagged event', () => {
    render(<ProductPreview />);
    expect(screen.getByText(/Clock synchronization requires verification/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Show in timeline' }));
    expect(screen.getByRole('tab', { name: /event timeline/i })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('button', { name: /Gate B opens · vehicle present/ })).toHaveAttribute('aria-pressed', 'true');
  });
});
