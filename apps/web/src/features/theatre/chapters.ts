import type { CaseSection } from '../../router/router';
import { formatDuration, type TheatreModel } from './model';

export type ChapterKey = 'seal' | 'clock' | 'place' | 'link' | 'conflict' | 'brief';

export interface Chapter {
  key: ChapterKey;
  number: string;
  short: string;
  title: string;
  /** Milliseconds of playback at 1×. */
  duration: number;
  /** Workspace view that shows this step in full. */
  section: CaseSection;
  what: (m: TheatreModel) => string;
  why: string;
  cli: (caseId: string) => string;
}

function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

export const CHAPTERS: Chapter[] = [
  {
    key: 'seal',
    number: '01',
    short: 'Seal',
    title: 'Sealed before anything else',
    duration: 8000,
    section: 'evidence',
    what: (m) =>
      `${plural(m.stats.exhibits, 'exhibit')} hashed with SHA-256 the moment ${m.stats.exhibits === 1 ? 'it' : 'they'} arrived. Originals stay read-only; every later step works on derived copies, and each custody entry carries the hash of the one before it.`,
    why: 'Change one byte and the hash changes. Chain of custody becomes something a court can check, not something it has to trust.',
    cli: (id) => `yukti custody verify --case ${id}`,
  },
  {
    key: 'clock',
    number: '02',
    short: 'Clock',
    title: 'Every source on one clock',
    duration: 9500,
    section: 'timeline',
    what: (m) => {
      const shifts = m.events.map((e) => Math.abs(e.wall - e.rawWall)).filter((s) => s > 30);
      const biggest = shifts.length ? Math.max(...shifts) : 0;
      const corrected = shifts.length
        ? ` ${plural(shifts.length, 'value')} moved when corrected, by up to ${formatDuration(biggest).replace(/^[+−]/, '')}.`
        : ' No source needed a time-zone or clock correction.';
      return `${plural(m.stats.events, 'event')} from ${plural(m.lanes.length, 'source')} were written on different clocks and in different zones. YUKTI normalises each value once and keeps its uncertainty window.${corrected}`;
    },
    why: 'Most false leads in multi-source cases are clock errors, not lies. A camera five minutes fast can look exactly like an alibi breaking.',
    cli: (id) => `yukti timeline --case ${id}`,
  },
  {
    key: 'place',
    number: '03',
    short: 'Place',
    title: 'Where the evidence puts them',
    duration: 8500,
    section: 'spatial',
    what: (m) =>
      m.places.length
        ? `${plural(m.places.length, 'place')} plotted and the route drawn in event order. Locations inferred from a tower or Wi-Fi coverage are drawn as areas, never as precise points.`
        : 'No exhibit carried a location. YUKTI does not invent one: events without a place stay off the plot and are listed as a gap.',
    why: 'A route turns a list of timestamps into something you can sanity-check: could anyone actually have made that journey?',
    cli: (id) => `yukti spatial --case ${id}`,
  },
  {
    key: 'link',
    number: '04',
    short: 'Link',
    title: 'Connections, each with a basis',
    duration: 8500,
    section: 'graph',
    what: (m) =>
      `${plural(m.nodes.length, 'node')} joined by ${plural(m.edges.length, 'link')}. A solid line was observed in a record; a dashed line was inferred by correlation; a red line marks a contradiction between records.`,
    why: 'Similarity is never shown as identity. Every link can be opened to the exhibits it rests on.',
    cli: (id) => `yukti graph summary --case ${id}`,
  },
  {
    key: 'conflict',
    number: '05',
    short: 'Conflict',
    title: 'What does not add up',
    duration: 9500,
    section: 'contradictions',
    what: (m) =>
      m.conflict
        ? `${plural(m.stats.leads, 'contradiction lead')} surfaced. Shown here: “${m.conflict.title}”. YUKTI puts the two records side by side and shows the arithmetic, then states what would resolve it.`
        : 'The reconstruction found no records that contradict each other. That statement covers these exhibits only; the gaps that limit it are listed in the report.',
    why: 'Contradictions are where investigations turn. Surfacing them early, with their basis, saves days of manual cross-checking.',
    cli: (id) => `yukti contradictions --case ${id}`,
  },
  {
    key: 'brief',
    number: '06',
    short: 'Brief',
    title: 'A lead, not a verdict',
    duration: 9000,
    section: 'reports',
    what: (m) =>
      `${plural(m.stats.assumptions, 'assumption')} labelled and ${plural(m.stats.leads, 'lead')} queued for a human. The judicial report carries every limit alongside every finding.`,
    why: 'Built for evidence that has to survive cross-examination: transparent, reproducible, and honest about what it does not know.',
    cli: (id) => `yukti report --case ${id} --out dossier.md`,
  },
];

export const TOTAL_DURATION = CHAPTERS.reduce((sum, c) => sum + c.duration, 0);

export const CHAPTER_STARTS: number[] = CHAPTERS.reduce<number[]>((starts, _chapter, i) => {
  starts.push(i === 0 ? 0 : starts[i - 1]! + CHAPTERS[i - 1]!.duration);
  return starts;
}, []);

/** Which chapter `time` falls in and how far through it (0..1). */
export function locate(time: number): { index: number; progress: number } {
  for (let i = CHAPTERS.length - 1; i >= 0; i -= 1) {
    if (time >= CHAPTER_STARTS[i]!) {
      return { index: i, progress: Math.min((time - CHAPTER_STARTS[i]!) / CHAPTERS[i]!.duration, 1) };
    }
  }
  return { index: 0, progress: 0 };
}
