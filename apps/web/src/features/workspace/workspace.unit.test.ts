import { describe, expect, it } from 'vitest';
import { SYNTHETIC_CASES, SYNTHETIC_CASE_DETAILS } from '../../fixtures/synthetic/workspace';
import { CASE_SECTIONS, caseHref, matchWorkspaceRoute } from '../../router/router';
import { formatBytes, formatEventTime, shortHash, durationLabel } from './format';
import { buildGraph, shortestPath } from './graph/graphModel';
import { sha256 } from './pages/EvidenceUploadPanel';

const detail = SYNTHETIC_CASE_DETAILS['CASE-026']!;

describe('matchWorkspaceRoute / caseHref', () => {
  it.each([
    ['/workspace', { kind: 'home' }],
    ['/workspace/settings', { kind: 'settings' }],
    ['/workspace/help', { kind: 'help' }],
    ['/workspace/cases/CASE-026', { kind: 'case', caseId: 'CASE-026', section: 'overview' }],
    ['/workspace/cases/CASE-026/graph', { kind: 'case', caseId: 'CASE-026', section: 'graph' }],
    ['/workspace/cases/CASE-026/bogus', { kind: 'not-found' }],
    ['/workspace/cases', { kind: 'not-found' }],
    ['/workspace/whatever', { kind: 'not-found' }],
  ])('%s', (path, expected) => {
    expect(matchWorkspaceRoute(path)).toEqual(expected);
  });

  it('ignores paths outside the workspace', () => {
    expect(matchWorkspaceRoute('/')).toBeNull();
    expect(matchWorkspaceRoute('/workspaces')).toBeNull();
  });

  it('builds round-trippable hrefs for every section', () => {
    for (const section of CASE_SECTIONS) {
      const href = caseHref('CASE-026', section);
      expect(matchWorkspaceRoute(href)).toEqual({ kind: 'case', caseId: 'CASE-026', section });
    }
    expect(caseHref('CASE-026', 'evidence', { evidence: 'EV-01' })).toBe('/workspace/cases/CASE-026/evidence?evidence=EV-01');
  });
});

describe('synthetic workspace fixture', () => {
  it('uses only canonical enum values (or explicit nulls)', () => {
    const caseStatuses = ['DRAFT', 'ACTIVE', 'UNDER_REVIEW', 'ARCHIVED'];
    const evidenceTypes = ['CCTV', 'CDR', 'CHAT_EXPORT', 'DOCUMENT_SCAN', 'DISK_IMAGE', 'AUDIO_RECORDING', 'FINANCIAL_LEDGER'];
    const entityTypes = ['PERSON', 'PHONE_NUMBER', 'VEHICLE', 'LOCATION', 'ORGANIZATION', 'DEVICE', 'CRYPTO_WALLET'];
    const relationTypes = ['COMMUNICATED_WITH', 'PRESENT_AT', 'CO_OCCURRED_WITH', 'OWNS', 'ASSOCIATE_OF', 'CONTRADICTS'];
    const precisions = ['EXACT_SECOND', 'MINUTE', 'HOUR', 'DAY', 'ESTIMATED'];
    for (const c of SYNTHETIC_CASES) expect(caseStatuses).toContain(c.status);
    for (const e of detail.evidence) if (e.source_type !== null) expect(evidenceTypes).toContain(e.source_type);
    for (const e of detail.entities) if (e.entity_type !== null) expect(entityTypes).toContain(e.entity_type);
    for (const r of detail.relationships) expect(relationTypes).toContain(r.relation_type);
    for (const e of detail.events) expect(precisions).toContain(e.timestamp_precision);
  });

  it('never fabricates weights or confidence', () => {
    expect(detail.relationships.every((r) => r.weight === null)).toBe(true);
    expect(detail.entities.every((e) => e.confidence === null)).toBe(true);
  });

  it('marks inferred relationships as not observed', () => {
    for (const r of detail.relationships) expect(r.is_observed).toBe(r.provenance !== 'inferred');
  });

  it('has 64-hex SHA-256 digests and summary counts that match the records', () => {
    for (const e of detail.evidence) expect(e.sha256_hash).toMatch(/^[0-9a-f]{64}$/);
    expect(detail.case.evidence_count).toBe(detail.evidence.length);
    expect(detail.case.event_count).toBe(detail.events.length);
    expect(detail.case.review_count).toBe(detail.review_items.filter((i) => i.status !== 'verified').length);
  });
});

describe('graph model', () => {
  const graph = buildGraph(detail);

  it('builds entity, event and evidence nodes with structural links', () => {
    expect(graph.nodes).toHaveLength(detail.entities.length + detail.events.length + detail.evidence.length);
    expect(graph.edges.filter((e) => e.kind === 'SOURCE_OF')).toHaveLength(detail.events.length);
    expect(graph.edges.some((e) => e.kind === 'CONTRADICTS' && e.source === 'EVT-3' && e.target === 'EVT-4')).toBe(true);
  });

  it('finds the shortest path and reports unreachable nodes', () => {
    expect(shortestPath(graph.edges, 'ENT-PA', 'ENT-PA')).toEqual([]);
    expect(shortestPath(graph.edges, 'ENT-PA', 'ENT-BADGE')).toEqual(['R1']);
    expect(shortestPath(graph.edges, 'ENT-ACC', 'ENT-LOC')?.length).toBe(3);
    expect(shortestPath(graph.edges.filter((e) => e.source !== 'ENT-ACC' && e.target !== 'ENT-ACC'), 'ENT-ACC', 'ENT-PA')).toBeNull();
  });
});

describe('format helpers', () => {
  it('never shows more precision than recorded', () => {
    const iso = '2026-03-14T16:28:00Z';
    expect(formatEventTime(iso, 'EXACT_SECOND')).toBe('21:58:00');
    expect(formatEventTime(iso, 'MINUTE')).toBe('21:58');
    expect(formatEventTime(iso, 'HOUR')).toBe('21:—');
    expect(formatEventTime(iso, 'ESTIMATED')).toBe('≈ 21:58');
    expect(formatEventTime(iso, 'DAY')).toBe('14 Mar 2026');
  });

  it('formats sizes, hashes and durations', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2_184_330)).toBe('2.1 MB');
    expect(shortHash('9f2c4be1d07a35c8e6f19b2a7d50c3e8814f6a2bd9e07c51f3a8e26b4d17a41e')).toBe('9f2c…a41e');
    expect(durationLabel('2026-03-14T17:11:07Z', '2026-03-14T17:13:52Z')).toBe('+2 min 45 s');
    expect(durationLabel('2026-03-14T17:13:52Z', '2026-03-14T17:13:34Z')).toBe('−18 s');
  });
});

describe('local SHA-256', () => {
  it('matches the standard test vector for "abc"', async () => {
    const file = new File(['abc'], 'abc.txt', { type: 'text/plain' });
    await expect(sha256(file)).resolves.toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
  });
});
