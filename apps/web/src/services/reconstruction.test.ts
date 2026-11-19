import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  adaptContradictionLeadsToReviewItems,
  adaptTimelineEntriesToWorkspaceEvents,
  getCaseContradictions,
  getCaseReconstruction,
  getCaseSpatial,
  getCaseTimeline,
  normalizeCaseId,
  triggerCaseReconstruction,
  type ContradictionLeadDto,
  type TimelineEntryDto,
} from './reconstruction';

describe('reconstruction service', () => {
  describe('normalizeCaseId', () => {
    it('preserves valid lowercase UUIDs', () => {
      const uuid = '550e8400-e29b-41d4-a716-446655440000';
      expect(normalizeCaseId(uuid)).toBe(uuid);
    });

    it('normalizes uppercase UUIDs to lowercase', () => {
      const uuid = '550E8400-E29B-41D4-A716-446655440000';
      expect(normalizeCaseId(uuid)).toBe(uuid.toLowerCase());
    });

    it('translates synthetic case slugs to deterministic UUID format', () => {
      expect(normalizeCaseId('CASE-026')).toBe('00000000-0000-0000-0000-000000000026');
      expect(normalizeCaseId('case-1')).toBe('00000000-0000-0000-0000-000000000001');
    });

    it('falls back to default UUID for unrecognized strings', () => {
      expect(normalizeCaseId('custom-unmatched')).toBe('00000000-0000-0000-0000-000000000001');
    });
  });

  describe('view model adapters', () => {
    it('adapts backend timeline entries to workspace events', () => {
      const entry: TimelineEntryDto = {
        event_id: 'evt-100',
        source_evidence_id: 'evi-500',
        time_window: {
          nominal_utc: '2026-11-18T10:00:00Z',
          precision: 'EXACT_SECOND',
          raw_precision_input: 'EXACT_SECOND',
          window_start_utc: '2026-11-18T10:00:00Z',
          window_end_utc: '2026-11-18T10:00:01Z',
          uncertainty_span_seconds: 1.0,
          has_explicit_bounds: false,
          is_estimated_window_assumption: false,
          source_timezone: 'UTC',
          clock_offset_seconds: 0.0,
        },
        raw_description: 'Saket Cell Tower Call',
        entity_ids: ['+919811002233', '+919899887766'],
        metadata: {
          call_type: 'VOICE_OUTGOING',
          call_timestamp: '2026-11-18T10:00:00Z',
        },
        ordering_status: 'STRICTLY_ORDERED',
        concurrent_event_ids: [],
      };

      const events = adaptTimelineEntriesToWorkspaceEvents([entry]);
      expect(events).toHaveLength(1);
      const ev = events[0]!;
      expect(ev.event_id).toBe('evt-100');
      expect(ev.title).toBe('Saket Cell Tower Call');
      expect(ev.event_type).toBe('TELEPHONY_VOICE_OUTGOING');
      expect(ev.timestamp).toBe('2026-11-18T10:00:00Z');
      expect(ev.timestamp_precision).toBe('EXACT_SECOND');
      expect(ev.ordering_status).toBe('STRICTLY_ORDERED');
      expect(ev.uncertainty?.window_start_utc).toBe('2026-11-18T10:00:00Z');
      expect(ev.uncertainty?.window_end_utc).toBe('2026-11-18T10:00:01Z');
      expect(ev.provenance).toBe('observed');
    });

    it('adapts backend contradiction leads to workspace review items', () => {
      const lead: ContradictionLeadDto = {
        lead_id: 'lead-001',
        lead_type: 'SPATIAL_TRANSIT_IMPOSSIBLE',
        entity_id: '+919811002233',
        event_a_id: 'evt-100',
        event_b_id: 'evt-101',
        observed_basis: {
          origin_event_id: 'evt-100',
          destination_event_id: 'evt-101',
          origin_evidence_id: 'evi-1',
          destination_evidence_id: 'evi-2',
          origin_timestamp_raw: '2026-11-18 10:00:00',
          destination_timestamp_raw: '2026-11-18 10:05:00',
        },
        derived_inference: {
          post_activity_min_required_speed_kmh: 450.0,
          reference_speed_threshold_kmh: 180.0,
          max_possible_elapsed_seconds: 300.0,
          min_possible_elapsed_seconds: 240.0,
          nominal_elapsed_seconds: 300.0,
        },
        neutral_explanation: 'Physical transit requires min speed 450.0 km/h exceeding 180.0 km/h threshold.',
        limitations: ['Check uncalibrated clock offset', 'Coarse cell tower centroid'],
        generated_at: '2026-11-18T12:00:00Z',
      };

      const reviews = adaptContradictionLeadsToReviewItems([lead]);
      expect(reviews).toHaveLength(1);
      const r = reviews[0]!;
      expect(r.id).toBe('lead-001');
      expect(r.category).toBe('conflicting');
      expect(r.status).toBe('flagged');
      expect(r.related_event_ids).toEqual(['evt-100', 'evt-101']);
      expect(r.related_entity_ids).toEqual(['+919811002233']);
      expect(r.records).toHaveLength(2);
      expect(r.limitations).toHaveLength(2);
    });
  });

  describe('API endpoints', () => {
    beforeEach(() => {
      vi.stubGlobal('fetch', vi.fn());
    });

    afterEach(() => {
      vi.unstubAllGlobals();
    });

    it('triggerCaseReconstruction dispatches POST to the correct endpoint', async () => {
      const mockResult = {
        case_id: '00000000-0000-0000-0000-000000000026',
        timeline: { total_events: 0, entries: [], overlaps: [], strictly_ordered_count: 0, concurrent_uncertain_count: 0 },
        spatial: { trajectories: [], overlaps: [] },
        contradictions: { leads: [], total_evaluated_segments: 0, flagged_segments_count: 0, disclaimer: '' },
      };

      (fetch as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
        ok: true,
        status: 200,
        headers: new Headers({ 'Content-Type': 'application/json' }),
        text: async () => JSON.stringify(mockResult),
        json: async () => mockResult,
      });

      const res = await triggerCaseReconstruction('CASE-026', { speed_threshold_kmh: 150 });
      expect(res.case_id).toBe('00000000-0000-0000-0000-000000000026');
      const call = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(call[0].toString()).toContain('/api/v1/cases/00000000-0000-0000-0000-000000000026/reconstruction');
      expect(call[1].method).toBe('POST');
    });

    it('getCaseReconstruction dispatches GET to reconstruction endpoint', async () => {
      (fetch as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
        ok: true,
        status: 200,
        headers: new Headers({ 'Content-Type': 'application/json' }),
        text: async () => JSON.stringify({ timeline: { entries: [] } }),
        json: async () => ({ timeline: { entries: [] } }),
      });

      await getCaseReconstruction('CASE-026');
      const call = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(call[0].toString()).toContain('/api/v1/cases/00000000-0000-0000-0000-000000000026/reconstruction');
      expect(call[1]?.method ?? 'GET').toBe('GET');
    });

    it('getCaseTimeline dispatches GET to timeline endpoint', async () => {
      (fetch as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
        ok: true,
        status: 200,
        headers: new Headers({ 'Content-Type': 'application/json' }),
        text: async () => JSON.stringify({ total_events: 5, entries: [] }),
        json: async () => ({ total_events: 5, entries: [] }),
      });

      await getCaseTimeline('CASE-026');
      const call = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(call[0].toString()).toContain('/api/v1/cases/00000000-0000-0000-0000-000000000026/timeline');
    });

    it('getCaseSpatial dispatches GET to spatial endpoint', async () => {
      (fetch as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
        ok: true,
        status: 200,
        headers: new Headers({ 'Content-Type': 'application/json' }),
        text: async () => JSON.stringify({ trajectories: [], overlaps: [] }),
        json: async () => ({ trajectories: [], overlaps: [] }),
      });

      await getCaseSpatial('CASE-026');
      const call = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(call[0].toString()).toContain('/api/v1/cases/00000000-0000-0000-0000-000000000026/spatial');
    });

    it('getCaseContradictions dispatches GET to contradictions endpoint', async () => {
      (fetch as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
        ok: true,
        status: 200,
        headers: new Headers({ 'Content-Type': 'application/json' }),
        text: async () => JSON.stringify({ leads: [], total_evaluated_segments: 0 }),
        json: async () => ({ leads: [], total_evaluated_segments: 0 }),
      });

      await getCaseContradictions('CASE-026');
      const call = (fetch as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(call[0].toString()).toContain('/api/v1/cases/00000000-0000-0000-0000-000000000026/contradictions');
    });
  });
});
