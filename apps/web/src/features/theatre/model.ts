/**
 * Reconstruction Theatre view model.
 *
 * Everything the theatre draws is derived here, once, from a loaded `CaseDetail` (synthetic or
 * live). The scenes stay pure functions of this model plus a 0..1 progress value, which is what
 * makes the whole playback scrubbable and deterministic.
 */
import type { CaseDetail, ProvenanceClass, ReviewItem, TransitBasis, WorkspaceEvent } from '../workspace/types';

/** Case clocks are displayed in India Standard Time (UTC+05:30). */
export const DISPLAY_OFFSET_SECONDS = 19_800;
export const DISPLAY_ZONE = 'IST';

const MAX_EXHIBITS = 6;
const MAX_EVENTS = 14;
const MAX_LANES = 6;
const MAX_NODES = 18;

export interface TExhibit {
  id: string;
  name: string;
  type: string;
  sha256: string;
  size: number;
  provenance: ProvenanceClass;
  source: string;
}

export interface TEvent {
  id: string;
  index: number;
  title: string;
  type: string;
  /** Seconds of the day on the display clock, after normalisation. */
  wall: number;
  /** Seconds of the day exactly as the source recorded it (same position if unparseable). */
  rawWall: number;
  /** Uncertainty window on the display clock, seconds of the day. */
  windowStart: number;
  windowEnd: number;
  raw: string;
  zone: string;
  lane: number;
  evidenceId: string;
  provenance: ProvenanceClass;
  flagged: boolean;
  uncertain: boolean;
  assumptions: number;
  placeId: string | null;
}

export interface TLane {
  evidenceId: string;
  label: string;
  type: string;
}

export interface TPlace {
  id: string;
  name: string;
  x: number;
  y: number;
  radius: number;
  kind: string;
  inferred: boolean;
  eventIndices: number[];
}

export type NodeKind = 'person' | 'device' | 'vehicle' | 'location' | 'account' | 'event' | 'evidence' | 'other';

export interface TNode {
  id: string;
  label: string;
  kind: NodeKind;
  x: number;
  y: number;
  degree: number;
}

export interface TEdge {
  id: string;
  from: string;
  to: string;
  label: string;
  observed: boolean;
  contradiction: boolean;
}

export interface TConflict {
  id: string;
  title: string;
  summary: string;
  a: { label: string; value: string; evidenceId: string };
  b: { label: string; value: string; evidenceId: string };
  discrepancy: string;
  action: string;
  limitations: string[];
  eventIndices: number[];
  /** Seconds between the two related events on the normalised clock, if both are known. */
  gapSeconds: number | null;
  transit: TransitBasis | null;
}

export interface TheatreModel {
  caseId: string;
  title: string;
  fir: string | null;
  exhibits: TExhibit[];
  exhibitTotal: number;
  lanes: TLane[];
  events: TEvent[];
  eventTotal: number;
  /** Display-clock range covering raw and normalised positions, padded. */
  clock: { start: number; end: number };
  /** Display-clock range of the normalised values only: the axis zooms to it after correction. */
  clockNormalised: { start: number; end: number };
  places: TPlace[];
  /** Place ids in event order: the route the evidence implies. */
  route: string[];
  nodes: TNode[];
  edges: TEdge[];
  conflict: TConflict | null;
  stats: {
    exhibits: number;
    events: number;
    places: number;
    entities: number;
    links: number;
    inferredLinks: number;
    leads: number;
    assumptions: number;
    uncertain: number;
  };
}

/* ------------------------------------------------------------------ clocks */

const DAY = 86_400;

function mod(value: number, by: number): number {
  return ((value % by) + by) % by;
}

/** Seconds of the day on the display clock for an ISO UTC timestamp. */
export function displayWall(isoUtc: string): number | null {
  const ms = Date.parse(isoUtc);
  if (Number.isNaN(ms)) return null;
  return mod(Math.floor(ms / 1000) + DISPLAY_OFFSET_SECONDS, DAY);
}

/** First `HH:MM[:SS]` in a recorded value, as seconds of the day, exactly as written. */
export function parseRecordedWall(raw: string): number | null {
  const match = /(?:^|[^\d])(\d{1,2}):(\d{2})(?::(\d{2}))?/.exec(raw);
  if (!match) return null;
  const h = Number(match[1]);
  const m = Number(match[2]);
  const s = match[3] ? Number(match[3]) : 0;
  if (h > 23 || m > 59 || s > 59) return null;
  return h * 3600 + m * 60 + s;
}

export function formatWall(seconds: number, withSeconds = true): string {
  const t = mod(Math.round(seconds), DAY);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  const pad = (n: number): string => String(n).padStart(2, '0');
  return withSeconds ? `${pad(h)}:${pad(m)}:${pad(s)}` : `${pad(h)}:${pad(m)}`;
}

export function formatDuration(seconds: number): string {
  const sign = seconds < 0 ? '−' : '+';
  let t = Math.abs(Math.round(seconds));
  const h = Math.floor(t / 3600);
  t -= h * 3600;
  const m = Math.floor(t / 60);
  const s = t - m * 60;
  if (h > 0) return `${sign}${h} h ${String(m).padStart(2, '0')} min`;
  if (m > 0) return `${sign}${m} min ${String(s).padStart(2, '0')} s`;
  return `${sign}${s} s`;
}

/* ------------------------------------------------------------------ graph layout */

/** Small deterministic Fruchterman–Reingold layout (no randomness: every render is identical). */
export function layoutGraph(
  ids: string[],
  links: { from: string; to: string }[],
  width: number,
  height: number,
): Map<string, { x: number; y: number }> {
  const n = ids.length;
  const cx = width / 2;
  const cy = height / 2;
  const pos = new Map<string, { x: number; y: number }>();
  ids.forEach((id, i) => {
    const angle = (i / Math.max(n, 1)) * Math.PI * 2 + 0.3;
    const ring = 0.35 + 0.15 * (i % 3);
    pos.set(id, { x: cx + Math.cos(angle) * width * ring, y: cy + Math.sin(angle) * height * ring });
  });
  const valid = links.filter((l) => pos.has(l.from) && pos.has(l.to) && l.from !== l.to);
  const k = Math.sqrt((width * height) / Math.max(n, 1)) * 0.62;
  const iterations = 420;
  for (let iter = 0; iter < iterations; iter += 1) {
    const temperature = (width / 9) * (1 - iter / iterations) + 0.5;
    const disp = new Map<string, { x: number; y: number }>(ids.map((id) => [id, { x: 0, y: 0 }]));
    for (let i = 0; i < n; i += 1) {
      for (let j = i + 1; j < n; j += 1) {
        const a = pos.get(ids[i]!)!;
        const b = pos.get(ids[j]!)!;
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let d = Math.hypot(dx, dy);
        if (d < 0.01) {
          dx = 0.5 * (i - j);
          dy = 0.5;
          d = Math.hypot(dx, dy);
        }
        const force = (k * k) / d;
        const da = disp.get(ids[i]!)!;
        const db = disp.get(ids[j]!)!;
        da.x += (dx / d) * force;
        da.y += (dy / d) * force;
        db.x -= (dx / d) * force;
        db.y -= (dy / d) * force;
      }
    }
    for (const link of valid) {
      const a = pos.get(link.from)!;
      const b = pos.get(link.to)!;
      const dx = a.x - b.x;
      const dy = a.y - b.y;
      const d = Math.max(Math.hypot(dx, dy), 0.01);
      const force = (d * d) / k;
      const da = disp.get(link.from)!;
      const db = disp.get(link.to)!;
      da.x -= (dx / d) * force;
      da.y -= (dy / d) * force;
      db.x += (dx / d) * force;
      db.y += (dy / d) * force;
    }
    for (const id of ids) {
      const p = pos.get(id)!;
      const d = disp.get(id)!;
      // Gravity keeps disconnected components on stage instead of drifting to the edges.
      d.x -= (p.x - cx) * 0.9;
      d.y -= (p.y - cy) * 1.4;
      const len = Math.max(Math.hypot(d.x, d.y), 0.01);
      p.x += (d.x / len) * Math.min(len, temperature);
      p.y += (d.y / len) * Math.min(len, temperature);
    }
  }
  // Fit the result into the box, keeping the aspect ratio.
  const xs = [...pos.values()].map((p) => p.x);
  const ys = [...pos.values()].map((p) => p.y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const scale = Math.min(width / Math.max(maxX - minX, 1), height / Math.max(maxY - minY, 1));
  const offX = (width - (maxX - minX) * scale) / 2;
  const offY = (height - (maxY - minY) * scale) / 2;
  const out = new Map<string, { x: number; y: number }>();
  for (const [id, p] of pos) {
    out.set(id, n === 1 ? { x: cx, y: cy } : { x: offX + (p.x - minX) * scale, y: offY + (p.y - minY) * scale });
  }
  return out;
}

/* ------------------------------------------------------------------ builders */

function shortLabel(text: string, max = 26): string {
  const clean = text.replace(/\s+\[[^\]]*\]$/, '').trim();
  return clean.length > max ? `${clean.slice(0, max - 1)}…` : clean;
}

/** Readable name for a live record description: the tower or site it was observed through. */
function placeName(text: string): string {
  const via = /\bvia\s+([A-Z0-9][\w-]+)/i.exec(text);
  if (via) return via[1]!;
  return shortLabel(text, 30);
}

/** Short label for an event node: its own title when it reads well, else time and type. */
function eventLabel(title: string, type: string, wall: number | null): string {
  if (title.length <= 26 && !/^Telephony\b/i.test(title)) return title;
  const kind = type.replace(/^TELEPHONY_/, '').replace(/_/g, ' ').toLowerCase();
  return wall === null ? shortLabel(kind, 22) : `${formatWall(wall, false)} · ${shortLabel(kind, 14)}`;
}

/** Record values that are ISO timestamps are shown on the display clock; others verbatim. */
export function displayValue(value: string): string {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(value)) return value;
  const wall = displayWall(value);
  return wall === null ? value : `${formatWall(wall)} ${DISPLAY_ZONE}`;
}

function nodeKind(type: string | null | undefined): NodeKind {
  switch (type) {
    case 'PERSON':
      return 'person';
    case 'DEVICE':
    case 'PHONE_NUMBER':
      return 'device';
    case 'VEHICLE':
      return 'vehicle';
    case 'LOCATION':
      return 'location';
    default:
      return type ? 'other' : 'account';
  }
}

function pickConflict(items: ReviewItem[]): ReviewItem | null {
  return items.find((i) => i.category === 'conflicting') ?? items.find((i) => i.status === 'flagged') ?? null;
}

function pickEvents(events: WorkspaceEvent[], keep: Set<string>): WorkspaceEvent[] {
  const sorted = [...events].sort((a, b) => a.timestamp.localeCompare(b.timestamp));
  if (sorted.length <= MAX_EVENTS) return sorted;
  const must = sorted.filter((e) => keep.has(e.event_id));
  const rest = sorted.filter((e) => !keep.has(e.event_id)).slice(0, MAX_EVENTS - must.length);
  return [...must, ...rest].sort((a, b) => a.timestamp.localeCompare(b.timestamp));
}

export function buildTheatreModel(detail: CaseDetail): TheatreModel {
  const conflictItem = pickConflict(detail.review_items);
  const flaggedIds = new Set(
    detail.review_items.filter((r) => r.category === 'conflicting').flatMap((r) => r.related_event_ids),
  );

  /* exhibits and lanes */
  const exhibits: TExhibit[] = detail.evidence.slice(0, MAX_EXHIBITS).map((e) => ({
    id: e.evidence_id,
    name: e.original_filename,
    type: e.source_type ?? e.source_type_label,
    sha256: e.sha256_hash,
    size: e.file_size_bytes,
    provenance: e.provenance,
    source: e.source,
  }));

  const chosen = pickEvents(detail.events, flaggedIds);
  const laneIds: string[] = [];
  for (const event of chosen) {
    if (!laneIds.includes(event.source_evidence_id)) laneIds.push(event.source_evidence_id);
  }
  const lanes: TLane[] = laneIds.slice(0, MAX_LANES).map((id) => {
    const ev = detail.evidence.find((e) => e.evidence_id === id);
    const type = ev ? ev.source_type ?? ev.source_type_label : '';
    const named = ev && ev.source && ev.source !== type && ev.source !== 'Observed exhibit' ? ev.source : ev?.original_filename;
    return {
      evidenceId: id,
      label: named ? shortLabel(named, 24) : shortLabel(id, 14),
      type: ev ? ev.source_type ?? ev.source_type_label : 'Exhibit',
    };
  });

  const placeOf = new Map<string, string>();
  for (const place of detail.places) for (const id of place.event_ids) if (!placeOf.has(id)) placeOf.set(id, place.place_id);

  const events: TEvent[] = chosen.map((e, index) => {
    const wall = displayWall(e.timestamp) ?? 0;
    const recorded = parseRecordedWall(e.source_recorded_time);
    const start = e.uncertainty ? displayWall(e.uncertainty.window_start_utc) : null;
    const end = e.uncertainty ? displayWall(e.uncertainty.window_end_utc) : null;
    const lane = Math.min(Math.max(laneIds.indexOf(e.source_evidence_id), 0), MAX_LANES - 1);
    return {
      id: e.event_id,
      index,
      title: shortLabel(e.title, 44),
      type: e.event_type,
      wall,
      rawWall: recorded ?? wall,
      windowStart: start ?? wall,
      windowEnd: end ?? wall,
      raw: e.source_recorded_time,
      zone: e.source_timezone,
      lane,
      evidenceId: e.source_evidence_id,
      provenance: e.provenance,
      flagged: flaggedIds.has(e.event_id),
      uncertain: e.ordering_status === 'CONCURRENT_UNCERTAIN',
      assumptions: e.assumptions?.length ?? 0,
      placeId: placeOf.get(e.event_id) ?? null,
    };
  });

  const allTimes = events.flatMap((e) => [e.wall, e.rawWall, e.windowStart, e.windowEnd]);
  const lo = allTimes.length ? Math.min(...allTimes) : 0;
  const hi = allTimes.length ? Math.max(...allTimes) : 3600;
  const pad = Math.max((hi - lo) * 0.08, 120);
  const clock = { start: lo - pad, end: hi + pad };
  const normTimes = events.flatMap((e) => [e.wall, e.windowStart, e.windowEnd]);
  const nlo = normTimes.length ? Math.min(...normTimes) : lo;
  const nhi = normTimes.length ? Math.max(...normTimes) : hi;
  const npad = Math.max((nhi - nlo) * 0.1, 90);
  const clockNormalised = { start: nlo - npad, end: nhi + npad };

  /* places and route */
  const eventIndex = new Map(events.map((e) => [e.id, e.index]));
  // Several records at the same coordinates are one place on the plot.
  const places: TPlace[] = [];
  const alias = new Map<string, string>();
  for (const p of detail.places) {
    const twin = places.find((o) => Math.abs(o.x - p.x) < 2 && Math.abs(o.y - p.y) < 2);
    const indices = p.event_ids.map((id) => eventIndex.get(id)).filter((i): i is number => i !== undefined);
    if (twin) {
      alias.set(p.place_id, twin.id);
      twin.eventIndices.push(...indices);
      continue;
    }
    places.push({
      id: p.place_id,
      name: placeName(p.name),
      x: p.x,
      y: p.y,
      radius: p.uncertainty_radius ?? 0,
      kind: p.kind,
      inferred: /INFERRED|CENTROID|not observed/i.test(p.note),
      eventIndices: indices,
    });
  }
  for (const e of events) if (e.placeId && alias.has(e.placeId)) e.placeId = alias.get(e.placeId)!;
  const route: string[] = [];
  for (const e of events) {
    if (e.placeId && route[route.length - 1] !== e.placeId) route.push(e.placeId);
  }

  /* graph */
  const label = new Map<string, { label: string; kind: NodeKind }>();
  for (const ent of detail.entities) label.set(ent.entity_id, { label: shortLabel(ent.canonical_name, 22), kind: nodeKind(ent.entity_type) });
  for (const ev of detail.events) {
    if (!label.has(ev.event_id)) label.set(ev.event_id, { label: eventLabel(ev.title, ev.event_type, displayWall(ev.timestamp)), kind: 'event' });
  }
  for (const ex of detail.evidence) if (!label.has(ex.evidence_id)) label.set(ex.evidence_id, { label: shortLabel(ex.original_filename, 22), kind: 'evidence' });

  let edges: TEdge[] = detail.relationships.map((r) => ({
    id: r.relationship_id,
    from: r.source_id,
    to: r.target_id,
    label: r.label,
    observed: r.is_observed,
    contradiction: r.relation_type === 'CONTRADICTS',
  }));
  if (edges.length > 0) {
    // Event nodes in the projected graph are joined to the entities they involve, so a
    // contradiction between two events sits inside the picture rather than floating beside it.
    const inGraph = new Set(edges.flatMap((e) => [e.from, e.to]));
    for (const ev of detail.events) {
      if (!inGraph.has(ev.event_id)) continue;
      for (const ent of ev.involved_entity_ids) {
        if (!label.has(ent)) continue;
        edges.push({ id: `${ev.event_id}→${ent}`, from: ev.event_id, to: ent, label: 'involves', observed: true, contradiction: false });
      }
    }
  }
  if (edges.length === 0) {
    // Live cases without a projected graph: link each event to the identifiers it involves.
    const derived: TEdge[] = [];
    for (const e of chosen) {
      for (const ent of e.involved_entity_ids) {
        if (!label.has(ent)) label.set(ent, { label: shortLabel(ent, 18), kind: /^\+?\d{6,}$/.test(ent) ? 'device' : 'other' });
        derived.push({ id: `${e.event_id}→${ent}`, from: e.event_id, to: ent, label: 'involves', observed: true, contradiction: false });
      }
    }
    if (conflictItem && conflictItem.related_event_ids.length >= 2) {
      const [a, b] = conflictItem.related_event_ids;
      derived.push({ id: `conflict-${conflictItem.id}`, from: a!, to: b!, label: 'contradiction lead', observed: false, contradiction: true });
    }
    edges = derived;
  }
  const degree = new Map<string, number>();
  for (const e of edges) {
    degree.set(e.from, (degree.get(e.from) ?? 0) + 1);
    degree.set(e.to, (degree.get(e.to) ?? 0) + 1);
  }
  const nodeIds = [...degree.keys()]
    .filter((id) => label.has(id))
    .sort((a, b) => (degree.get(b) ?? 0) - (degree.get(a) ?? 0))
    .slice(0, MAX_NODES);
  const nodeSet = new Set(nodeIds);
  edges = edges.filter((e) => nodeSet.has(e.from) && nodeSet.has(e.to));
  const layout = layoutGraph(nodeIds, edges, 760, 430);
  const nodes: TNode[] = nodeIds.map((id) => ({
    id,
    label: label.get(id)!.label,
    kind: label.get(id)!.kind,
    x: layout.get(id)!.x,
    y: layout.get(id)!.y,
    degree: degree.get(id) ?? 0,
  }));

  /* conflict */
  let conflict: TConflict | null = null;
  if (conflictItem) {
    const related = conflictItem.related_event_ids
      .map((id) => eventIndex.get(id))
      .filter((i): i is number => i !== undefined);
    const fullA = detail.events.find((e) => e.event_id === conflictItem.related_event_ids[0]);
    const fullB = detail.events.find((e) => e.event_id === conflictItem.related_event_ids[1]);
    const gap =
      fullA && fullB ? (Date.parse(fullB.timestamp) - Date.parse(fullA.timestamp)) / 1000 : null;
    const rec = (i: number): TConflict['a'] => {
      const r = conflictItem.records[i];
      return { label: r?.label ?? '—', value: r ? displayValue(r.value) : '—', evidenceId: r?.evidence_id ?? '' };
    };
    conflict = {
      id: conflictItem.id,
      title: conflictItem.title,
      summary: conflictItem.summary,
      a: rec(0),
      b: rec(1),
      discrepancy: conflictItem.discrepancy ?? conflictItem.summary,
      action: conflictItem.suggested_action,
      limitations: conflictItem.limitations,
      eventIndices: related,
      gapSeconds: gap !== null && Number.isFinite(gap) ? gap : null,
      transit: conflictItem.transit ?? null,
    };
  }

  const inferredLinks = detail.relationships.filter((r) => !r.is_observed).length;
  return {
    caseId: detail.case.case_id,
    title: detail.case.title,
    fir: detail.case.fir_number ?? null,
    exhibits,
    exhibitTotal: detail.evidence.length,
    lanes,
    events,
    eventTotal: detail.events.length,
    clock,
    clockNormalised,
    places,
    route,
    nodes,
    edges,
    conflict,
    stats: {
      exhibits: detail.evidence.length,
      events: detail.events.length,
      places: detail.places.length,
      entities: detail.entities.length || nodes.filter((n) => n.kind !== 'event' && n.kind !== 'evidence').length,
      links: detail.relationships.length || edges.length,
      inferredLinks,
      leads: detail.review_items.filter((r) => r.category === 'conflicting').length,
      assumptions: detail.events.reduce((sum, e) => sum + (e.assumptions?.length ?? 0), 0),
      uncertain: detail.events.filter((e) => e.ordering_status === 'CONCURRENT_UNCERTAIN').length,
    },
  };
}
