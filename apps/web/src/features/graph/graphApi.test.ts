import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  fetchEntityNeighbors,
  fetchEntityPath,
  fetchGraphSummary,
  normalizeCaseId,
  parseGraphNeighbors,
  parseGraphPath,
  parseGraphSummary,
} from './services/graphApi';

describe('Graph API Client & Parsers', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  describe('normalizeCaseId', () => {
    it('preserves valid standard RFC 4122 UUIDs', () => {
      const validUuid = '123e4567-e89b-12d3-a456-426614174000';
      expect(normalizeCaseId(validUuid)).toBe(validUuid);
    });

    it('translates synthetic case slugs to deterministic UUID format', () => {
      expect(normalizeCaseId('CASE-026')).toBe('00000000-0000-0000-0000-000000000026');
      expect(normalizeCaseId('CASE-999')).toBe('00000000-0000-0000-0000-000000000999');
    });

    it('falls back safely for unparseable case strings', () => {
      expect(normalizeCaseId('UNKNOWN')).toBe('00000000-0000-0000-0000-000000000001');
    });
  });

  describe('DTO Runtime Parsers', () => {
    it('parses valid GraphSummaryDto', () => {
      const raw = {
        case_id: 'a1b2c3d4-e5f6-7890-abcd-ef1234567890',
        evidence_count: 3,
        extraction_run_count: 2,
        entity_candidate_count: 14,
        event_candidate_count: 8,
        relationship_assertion_count: 12,
        reconstruction_run_count: 1,
        contradiction_lead_count: 1,
        graph_synchronization_status: 'SYNCHRONIZED',
        last_projection_time: '2026-11-19T10:00:00Z',
        identifier_privacy: 'hmac_sha256_active',
      };
      const parsed = parseGraphSummary(raw);
      expect(parsed.case_id).toBe('a1b2c3d4-e5f6-7890-abcd-ef1234567890');
      expect(parsed.entity_candidate_count).toBe(14);
      expect(parsed.graph_synchronization_status).toBe('SYNCHRONIZED');
    });

    it('rejects malformed GraphSummaryDto', () => {
      expect(() => parseGraphSummary(null)).toThrow(TypeError);
      expect(() => parseGraphSummary('invalid')).toThrow(TypeError);
    });

    it('parses valid GraphNeighborResponseDto', () => {
      const raw = {
        case_id: 'a1b2c3d4-e5f6-7890-abcd-ef1234567890',
        root_entity_id: 'ent-1',
        depth: 2,
        nodes: [{ id: 'ent-1', label: 'EntityCandidate', properties: {} }],
        relationships: [],
      };
      const parsed = parseGraphNeighbors(raw);
      expect(parsed.root_entity_id).toBe('ent-1');
      expect(parsed.depth).toBe(2);
      expect(parsed.nodes).toHaveLength(1);
    });

    it('parses valid GraphPathResponseDto', () => {
      const raw = {
        case_id: 'a1b2c3d4-e5f6-7890-abcd-ef1234567890',
        from_entity_id: 'ent-1',
        to_entity_id: 'ent-2',
        found: true,
        nodes: [],
        relationships: [],
      };
      const parsed = parseGraphPath(raw);
      expect(parsed.found).toBe(true);
      expect(parsed.from_entity_id).toBe('ent-1');
      expect(parsed.to_entity_id).toBe('ent-2');
    });
  });

  describe('fetchGraphSummary API request', () => {
    it('queries summary endpoint with normalized case UUID', async () => {
      const mockFetch = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () =>
          JSON.stringify({
            case_id: '00000000-0000-0000-0000-000000000026',
            evidence_count: 5,
            extraction_run_count: 5,
            entity_candidate_count: 20,
            event_candidate_count: 15,
            relationship_assertion_count: 18,
            reconstruction_run_count: 2,
            contradiction_lead_count: 1,
            graph_synchronization_status: 'SYNCHRONIZED',
            last_projection_time: null,
            identifier_privacy: 'hmac_sha256_active',
          }),
      });
      globalThis.fetch = mockFetch;

      const res = await fetchGraphSummary('CASE-026');
      expect(res.entity_candidate_count).toBe(20);
      expect(mockFetch).toHaveBeenCalledTimes(1);
      const url = mockFetch.mock.calls[0][0].toString();
      expect(url).toContain('/api/v1/cases/00000000-0000-0000-0000-000000000026/graph/summary');
    });
  });

  describe('fetchEntityNeighbors query parameter clamping', () => {
    it('clamps depth to [1, 3] and limit to [1, 100]', async () => {
      const mockFetch = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () =>
          JSON.stringify({
            case_id: '00000000-0000-0000-0000-000000000026',
            root_entity_id: 'ent-1',
            depth: 3,
            nodes: [],
            relationships: [],
          }),
      });
      globalThis.fetch = mockFetch;

      // Pass depth 99 and limit 500
      await fetchEntityNeighbors('CASE-026', 'ent-1', 99, 500);

      const url = mockFetch.mock.calls[0][0].toString();
      expect(url).toContain('depth=3');
      expect(url).toContain('limit=100');
    });
  });

  describe('fetchEntityPath query parameter clamping', () => {
    it('clamps max_hops to [1, 5]', async () => {
      const mockFetch = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () =>
          JSON.stringify({
            case_id: '00000000-0000-0000-0000-000000000026',
            from_entity_id: 'ent-1',
            to_entity_id: 'ent-2',
            found: false,
            nodes: [],
            relationships: [],
          }),
      });
      globalThis.fetch = mockFetch;

      // Pass max_hops 20
      await fetchEntityPath('CASE-026', 'ent-1', 'ent-2', 20);

      const url = mockFetch.mock.calls[0][0].toString();
      expect(url).toContain('max_hops=5');
      expect(url).toContain('from=ent-1');
      expect(url).toContain('to=ent-2');
    });
  });

  describe('Cancellation and Error Handling', () => {
    it('respects pre-aborted signal', async () => {
      const controller = new AbortController();
      controller.abort();

      await expect(
        fetchGraphSummary('CASE-026', { signal: controller.signal }),
      ).rejects.toThrow();
    });
  });
});
