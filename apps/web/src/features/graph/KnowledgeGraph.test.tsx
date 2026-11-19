import { act } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SYNTHETIC_CASE_DETAILS } from '../../fixtures/synthetic/workspace';
import { KnowledgeGraph } from './components/KnowledgeGraph';

describe('KnowledgeGraph Component (Phase 2B)', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('renders the knowledge graph workspace and loads summary', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () =>
        JSON.stringify({
          case_id: '00000000-0000-0000-0000-000000000026',
          evidence_count: 5,
          extraction_run_count: 5,
          entity_candidate_count: 6,
          event_candidate_count: 5,
          relationship_assertion_count: 8,
          reconstruction_run_count: 1,
          contradiction_lead_count: 1,
          graph_synchronization_status: 'SYNCHRONIZED',
          last_projection_time: '2026-11-19T08:00:00Z',
          identifier_privacy: 'hmac_sha256_active',
        }),
    });
    globalThis.fetch = mockFetch;

    const detail = SYNTHETIC_CASE_DETAILS['CASE-026'];
    render(<KnowledgeGraph detail={detail} />);

    // Check title and description
    expect(screen.getByText('Forensic Knowledge Graph')).toBeInTheDocument();

    // Check sync status badge
    await waitFor(() => {
      expect(screen.getByText(/GRAPH: SYNCHRONIZED/i)).toBeInTheDocument();
      expect(screen.getByText(/PRIVACY: HMAC-SHA256/i)).toBeInTheDocument();
    });

    // Check legend items
    expect(screen.getByText('Node Categories')).toBeInTheDocument();
    expect(screen.getByText('Observed Link')).toBeInTheDocument();
    expect(screen.getByText(/Forensic Safeguard/i)).toBeInTheDocument();
  });

  it('switches between interactive canvas and accessible table view', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () =>
        JSON.stringify({
          case_id: '00000000-0000-0000-0000-000000000026',
          evidence_count: 5,
          extraction_run_count: 5,
          entity_candidate_count: 6,
          event_candidate_count: 5,
          relationship_assertion_count: 8,
          reconstruction_run_count: 1,
          contradiction_lead_count: 1,
          graph_synchronization_status: 'SYNCHRONIZED',
          last_projection_time: null,
          identifier_privacy: 'hmac_sha256_active',
        }),
    });

    const detail = SYNTHETIC_CASE_DETAILS['CASE-026'];
    render(<KnowledgeGraph detail={detail} />);

    // Click Table view toggle
    const tableButton = screen.getByRole('button', { name: /Accessible tabular graph view/i });
    await act(async () => {
      fireEvent.click(tableButton);
    });

    // Verify table rendered
    expect(screen.getByText(/Graph Nodes List/i)).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'Category' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: 'Label' })).toBeInTheDocument();

    // Verify accessible inspect action
    const inspectButtons = screen.getAllByRole('button', { name: /Inspect/i });
    expect(inspectButtons.length).toBeGreaterThan(0);
    await act(async () => {
      fireEvent.click(inspectButtons[0]);
    });

    // Inspecting switches back to canvas and populates inspector
    expect(screen.getByText('Forensic Inspector')).toBeInTheDocument();
  });

  it('displays degraded status when graph subsystem is disabled', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () =>
        JSON.stringify({
          case_id: '00000000-0000-0000-0000-000000000026',
          evidence_count: 0,
          extraction_run_count: 0,
          entity_candidate_count: 0,
          event_candidate_count: 0,
          relationship_assertion_count: 0,
          reconstruction_run_count: 0,
          contradiction_lead_count: 0,
          graph_synchronization_status: 'GRAPH_DISABLED',
          last_projection_time: null,
          identifier_privacy: 'unconfigured',
        }),
    });

    const detail = SYNTHETIC_CASE_DETAILS['CASE-026'];
    render(<KnowledgeGraph detail={detail} />);

    await waitFor(() => {
      expect(screen.getByText(/GRAPH: GRAPH_DISABLED/i)).toBeInTheDocument();
    });
  });
});
