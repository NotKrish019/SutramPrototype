import { describe, expect, it } from 'vitest';
import {
  adaptBackendNodes,
  adaptBackendRelationships,
  adaptCaseDetailToCytoscape,
  deriveSafeNodeLabel,
  normalizeCategory,
} from './services/graphAdapter';
import type { BackendGraphNode, BackendGraphRelationship } from './types/graph';
import { SYNTHETIC_CASE_DETAILS } from '../../fixtures/synthetic/workspace';

describe('Graph Adapter', () => {
  describe('normalizeCategory', () => {
    it('maps all 10 Phase 2A node categories correctly', () => {
      expect(normalizeCategory('Case')).toBe('Case');
      expect(normalizeCategory('Evidence')).toBe('Evidence');
      expect(normalizeCategory('ExtractionRun')).toBe('ExtractionRun');
      expect(normalizeCategory('EntityCandidate')).toBe('EntityCandidate');
      expect(normalizeCategory('EventCandidate')).toBe('EventCandidate');
      expect(normalizeCategory('RelationshipAssertion')).toBe('RelationshipAssertion');
      expect(normalizeCategory('ReconstructionRun')).toBe('ReconstructionRun');
      expect(normalizeCategory('ContradictionLead')).toBe('ContradictionLead');
      expect(normalizeCategory('TemporalCluster')).toBe('TemporalCluster');
      expect(normalizeCategory('SpatialLocus')).toBe('SpatialLocus');
    });

    it('falls back safely to EntityCandidate for unknown categories', () => {
      expect(normalizeCategory('UnknownCategory')).toBe('EntityCandidate');
    });
  });

  describe('deriveSafeNodeLabel & Identifier Privacy', () => {
    it('uses canonical_name when available', () => {
      const node: BackendGraphNode = {
        id: 'ent-1',
        label: 'EntityCandidate',
        properties: { canonical_name: 'Pawan Arora', entity_type: 'PERSON' },
      };
      const res = deriveSafeNodeLabel(node);
      expect(res.label).toBe('Pawan Arora');
      expect(res.sublabel).toBe('PERSON');
      expect(res.isRedacted).toBe(false);
    });

    it('redacts sensitive technical identifiers with HMAC fingerprint representation', () => {
      const node: BackendGraphNode = {
        id: 'ent-dev',
        label: 'EntityCandidate',
        properties: {
          fingerprint: '7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069',
          entity_type: 'PHONE_NUMBER',
        },
      };
      const res = deriveSafeNodeLabel(node);
      expect(res.label).toContain('[HMAC]');
      expect(res.label).toContain('7f83b1…9069');
      expect(res.sublabel).toBe('PHONE_NUMBER:PROTECTED');
      expect(res.isRedacted).toBe(true);
    });

    it('handles event title and source hash correctly', () => {
      const eventNode: BackendGraphNode = {
        id: 'ev-1',
        label: 'EventCandidate',
        properties: { title: 'Call Initiated', event_type: 'CALL' },
      };
      expect(deriveSafeNodeLabel(eventNode).label).toBe('Call Initiated');

      const fileNode: BackendGraphNode = {
        id: 'file-1',
        label: 'Evidence',
        properties: {
          sha256_hash: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
          source_type: 'CDR',
        },
      };
      expect(deriveSafeNodeLabel(fileNode).label).toBe('e3b0c442…');
    });

    it('handles missing or null properties safely', () => {
      const node: BackendGraphNode = {
        id: 'fallback-node',
        label: 'CustomLabel',
        properties: {},
      };
      const res = deriveSafeNodeLabel(node);
      expect(res.label).toBe('fallback-node');
      expect(res.sublabel).toBe('CustomLabel');
    });
  });

  describe('adaptBackendNodes', () => {
    it('converts nodes and flags root entity', () => {
      const rawNodes: BackendGraphNode[] = [
        {
          id: 'ent-1',
          label: 'EntityCandidate',
          properties: { canonical_name: 'Alpha', confidence: 0.95 },
        },
        {
          id: 'ent-2',
          label: 'EntityCandidate',
          properties: { canonical_name: 'Beta' },
        },
      ];
      const elements = adaptBackendNodes(rawNodes, 'ent-1');
      expect(elements).toHaveLength(2);
      expect(elements[0].data.id).toBe('ent-1');
      expect((elements[0].data as any).isRoot).toBe(true);
      expect((elements[1].data as any).isRoot).toBe(false);
      expect((elements[0].data as any).confidence).toBe(0.95);
      expect((elements[1].data as any).confidence).toBeNull();
    });

    it('deduplicates nodes with identical IDs', () => {
      const rawNodes: BackendGraphNode[] = [
        { id: 'dup-1', label: 'EntityCandidate', properties: { canonical_name: 'First' } },
        { id: 'dup-1', label: 'EntityCandidate', properties: { canonical_name: 'Duplicate' } },
      ];
      const elements = adaptBackendNodes(rawNodes);
      expect(elements).toHaveLength(1);
      expect((elements[0].data as any).label).toBe('First');
    });
  });

  describe('adaptBackendRelationships', () => {
    it('converts relationships and identifies contradictions', () => {
      const rawRels: BackendGraphRelationship[] = [
        {
          id: 'rel-1',
          source: 'ent-1',
          target: 'ent-2',
          relation_type: 'COMMUNICATED_WITH',
          properties: { is_observed: true },
        },
        {
          id: 'rel-2',
          source: 'ev-1',
          target: 'ev-2',
          relation_type: 'CONTRADICTS_EVENT',
          properties: { is_observed: false },
        },
      ];
      const elements = adaptBackendRelationships(rawRels);
      expect(elements).toHaveLength(2);

      const e1 = elements[0].data as any;
      expect(e1.relationType).toBe('COMMUNICATED_WITH');
      expect(e1.isObserved).toBe(true);
      expect(e1.provenance).toBe('observed');
      expect(e1.isContradiction).toBe(false);

      const e2 = elements[1].data as any;
      expect(e2.relationType).toBe('CONTRADICTS_EVENT');
      expect(e2.isObserved).toBe(false);
      expect(e2.provenance).toBe('inferred');
      expect(e2.isContradiction).toBe(true);
    });

    it('deduplicates relationships with identical IDs', () => {
      const rawRels: BackendGraphRelationship[] = [
        { id: 'rel-dup', source: 'a', target: 'b', relation_type: 'KNOWS', properties: {} },
        { id: 'rel-dup', source: 'a', target: 'b', relation_type: 'KNOWS', properties: {} },
      ];
      const elements = adaptBackendRelationships(rawRels);
      expect(elements).toHaveLength(1);
    });
  });

  describe('adaptCaseDetailToCytoscape', () => {
    it('adapts synthetic CaseDetail completely', () => {
      const detail = SYNTHETIC_CASE_DETAILS['CASE-026'];
      expect(detail).toBeDefined();

      const elements = adaptCaseDetailToCytoscape(detail);
      const nodes = elements.filter((el) => el.group === 'nodes');
      const edges = elements.filter((el) => el.group === 'edges');

      expect(nodes.length).toBeGreaterThan(0);
      expect(edges.length).toBeGreaterThan(0);

      // Verify node categories exist
      const categories = new Set(nodes.map((n) => (n.data as any).category));
      expect(categories.has('EntityCandidate')).toBe(true);
      expect(categories.has('EventCandidate')).toBe(true);
      expect(categories.has('Evidence')).toBe(true);
    });
  });
});
