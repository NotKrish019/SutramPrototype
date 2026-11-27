import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { SYNTHETIC_CASE_DETAILS } from '../../fixtures/synthetic/workspace';
import type { CaseDetail } from '../workspace/types';
import { CHAPTERS, CHAPTER_STARTS, TOTAL_DURATION, locate } from './chapters';
import { buildTheatreModel, displayValue, formatDuration, formatWall, layoutGraph, parseRecordedWall } from './model';
import { ReconstructionTheatre } from './ReconstructionTheatre';

const detail = SYNTHETIC_CASE_DETAILS['CASE-026']!;

describe('theatre clocks', () => {
  it('reads the first wall-clock time exactly as a source wrote it', () => {
    expect(parseRecordedWall('14/03/2026 22:41:07 (camera overlay)')).toBe(22 * 3600 + 41 * 60 + 7);
    expect(parseRecordedWall('14-03-2026 21:58')).toBe(21 * 3600 + 58 * 60);
    expect(parseRecordedWall('2026-11-10T20:55:10+05:30')).toBe(20 * 3600 + 55 * 60 + 10);
    expect(parseRecordedWall('no time here')).toBeNull();
    expect(parseRecordedWall('99:99')).toBeNull();
  });

  it('formats walls, durations and ISO record values on the IST display clock', () => {
    expect(formatWall(3661)).toBe('01:01:01');
    expect(formatWall(-1)).toBe('23:59:59');
    expect(formatDuration(165)).toBe('+2 min 45 s');
    expect(formatDuration(-19_800)).toBe('−5 h 30 min');
    expect(displayValue('2026-11-10T15:25:10+00:00')).toBe('20:55:10 IST');
    expect(displayValue('22:41:07 IST')).toBe('22:41:07 IST');
  });
});

describe('buildTheatreModel', () => {
  const model = buildTheatreModel(detail);

  it('keeps every event, sorted, on lanes per source', () => {
    expect(model.events).toHaveLength(detail.events.length);
    const walls = model.events.map((e) => e.wall);
    expect([...walls].sort((a, b) => a - b)).toEqual(walls);
    expect(model.lanes.length).toBeGreaterThan(1);
  });

  it('shows the UTC controller record moving onto the IST clock by 5 h 30 min', () => {
    const controller = model.events.find((e) => e.id === 'EVT-4')!;
    expect(controller.wall - controller.rawWall).toBe(19_800);
    const camera = model.events.find((e) => e.id === 'EVT-3')!;
    expect(camera.wall).toBe(camera.rawWall);
  });

  it('picks the conflicting review item and measures its gap', () => {
    expect(model.conflict?.id).toBe('DX-01');
    expect(model.conflict?.gapSeconds).toBe(165);
    expect(model.events.filter((e) => e.flagged).map((e) => e.id).sort()).toEqual(['EVT-3', 'EVT-4']);
  });

  it('connects contradicting events into the graph rather than leaving them floating', () => {
    const contradiction = model.edges.find((e) => e.contradiction)!;
    expect(contradiction).toBeDefined();
    const touching = model.edges.filter((e) => !e.contradiction && (e.from === contradiction.from || e.to === contradiction.from));
    expect(touching.length).toBeGreaterThan(0);
  });

  it('derives a graph from involved identifiers when no relationships are projected', () => {
    const live: CaseDetail = { ...detail, relationships: [], entities: [] };
    const m = buildTheatreModel(live);
    expect(m.edges.length).toBeGreaterThan(0);
    expect(m.edges.every((e) => m.nodes.some((n) => n.id === e.from) && m.nodes.some((n) => n.id === e.to))).toBe(true);
  });

  it('handles an empty case without throwing', () => {
    const empty: CaseDetail = { ...detail, evidence: [], events: [], relationships: [], entities: [], places: [], review_items: [] };
    const m = buildTheatreModel(empty);
    expect(m.events).toEqual([]);
    expect(m.conflict).toBeNull();
    expect(m.nodes).toEqual([]);
  });
});

describe('layoutGraph', () => {
  it('is deterministic and stays inside the box', () => {
    const ids = ['a', 'b', 'c', 'd', 'e'];
    const links = [{ from: 'a', to: 'b' }, { from: 'b', to: 'c' }, { from: 'd', to: 'e' }];
    const first = layoutGraph(ids, links, 400, 300);
    const second = layoutGraph(ids, links, 400, 300);
    expect([...first.entries()]).toEqual([...second.entries()]);
    for (const p of first.values()) {
      expect(p.x).toBeGreaterThanOrEqual(-0.001);
      expect(p.x).toBeLessThanOrEqual(400.001);
      expect(p.y).toBeGreaterThanOrEqual(-0.001);
      expect(p.y).toBeLessThanOrEqual(300.001);
    }
  });
});

describe('chapters', () => {
  it('locates any time in exactly one chapter', () => {
    expect(locate(0)).toEqual({ index: 0, progress: 0 });
    expect(locate(CHAPTER_STARTS[2]! + CHAPTERS[2]!.duration / 2).index).toBe(2);
    expect(locate(TOTAL_DURATION)).toEqual({ index: CHAPTERS.length - 1, progress: 1 });
  });
});

describe('<ReconstructionTheatre />', () => {
  it('renders, narrates, and jumps between chapters from the keyboard and the rail', () => {
    render(<ReconstructionTheatre detail={detail} />);
    expect(screen.getByRole('heading', { level: 2, name: 'Sealed before anything else' })).toBeInTheDocument();
    act(() => {
      fireEvent.keyDown(window, { key: '5' });
    });
    expect(screen.getByRole('heading', { level: 2, name: 'What does not add up' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Clock/ }));
    expect(screen.getByRole('heading', { level: 2, name: 'Every source on one clock' })).toBeInTheDocument();
    expect(screen.getByRole('slider', { name: 'Playback position' })).toBeInTheDocument();
    // Synthetic web cases point at the CLI demo, which builds a live case.
    expect(screen.getByText('yukti demo')).toBeInTheDocument();
  });
});
