import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SYNTHETIC_CASE_DETAILS } from '../../../fixtures/synthetic/workspace';
import type { WorkspaceEvidence } from '../types';
import { EvidencePreview, hasSyntheticArtwork } from './EvidencePreview';

const ev01 = SYNTHETIC_CASE_DETAILS['CASE-026']!.evidence.find((e) => e.evidence_id === 'EV-01')!;

describe('EvidencePreview', () => {
  it('shows labelled synthetic artwork for registered demonstration artifacts', () => {
    render(<EvidencePreview evidence={ev01} synthetic />);
    expect(screen.getByRole('img', { name: 'Synthetic preview of EV-01' })).toBeInTheDocument();
    expect(screen.getByText('Synthetic')).toBeInTheDocument();
  });

  it('never shows demonstration artwork for evidence from a non-synthetic source', () => {
    render(<EvidencePreview evidence={ev01} synthetic={false} />);
    expect(screen.queryByRole('img', { name: /Synthetic preview/ })).not.toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'CCTV file, MP4' })).toBeInTheDocument();
  });

  it('falls back to a type glyph for artifacts without registered artwork', () => {
    const other: WorkspaceEvidence = { ...ev01, case_id: 'CASE-999', evidence_id: 'EV-77', source_type: null, source_type_label: 'Unclassified', original_filename: 'export.log' };
    expect(hasSyntheticArtwork(other)).toBe(false);
    render(<EvidencePreview evidence={other} synthetic />);
    expect(screen.getByRole('img', { name: 'Unclassified file, LOG' })).toBeInTheDocument();
  });

  it('falls back to a generic glyph for source types the UI does not know (e.g. TABULAR_CSV)', () => {
    const csv = {
      ...ev01,
      case_id: 'CASE-999',
      evidence_id: 'EV-78',
      source_type: 'TABULAR_CSV',
      source_type_label: 'Tabular CSV',
      original_filename: 'carrier_a_cdr.csv',
    } as unknown as WorkspaceEvidence;
    render(<EvidencePreview evidence={csv} synthetic />);
    expect(screen.getByRole('img', { name: 'Tabular CSV file, CSV' })).toBeInTheDocument();
  });
});
