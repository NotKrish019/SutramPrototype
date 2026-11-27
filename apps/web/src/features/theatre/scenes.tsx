/**
 * Theatre scenes. Each scene is a pure function of the model and its chapter progress `p`
 * (0..1), drawn into one shared 1200×660 SVG stage. No scene keeps state or timers.
 */
import React from 'react';
import { formatDuration, formatWall, type TEvent, type TheatreModel, type TNode } from './model';
import { clamp, easeInOut, easeOut, easeOutExpo, hexNoise, lerp, seg, wrap } from './motion';

export const STAGE_W = 1200;
export const STAGE_H = 660;

const MONO = '"JetBrains Mono", ui-monospace, monospace';
const DISPLAY = 'Archivo, system-ui, sans-serif';
const SERIF = 'Archivo, system-ui, sans-serif';
const SANS = 'Inter, system-ui, sans-serif';

const C = {
  bg: '#0A0A0B',
  panel: '#111113',
  line: '#26262B',
  strong: '#3A3A41',
  fg: '#EDEDEF',
  bright: '#FFFFFF',
  muted: '#A1A1AA',
  subtle: '#71717A',
  crimson: '#E5243B',
  deep: '#A3172B',
  amber: '#D9A441',
  ok: '#4FB37D',
  steel: '#8C9EB3',
  silver: '#C8CAD0',
};

const displayStyle: React.CSSProperties = { fontStretch: '118%' };

export interface SceneProps {
  model: TheatreModel;
  p: number;
  /** Seconds since playback started; drives ambient loops such as particles. */
  t: number;
  /** Draw only the picture: no legends or explanatory captions (embedded previews). */
  bare?: boolean;
}

/* ------------------------------------------------------------------ shared defs */

export const StageDefs: React.FC = () => (
  <defs>
    <pattern id="th-grid" width="24" height="24" patternUnits="userSpaceOnUse">
      <path d="M24 0H0V24" fill="none" stroke="#17171a" strokeWidth="1" />
    </pattern>
    <pattern id="th-grid-major" width="120" height="120" patternUnits="userSpaceOnUse">
      <rect width="120" height="120" fill="url(#th-grid)" />
      <path d="M120 0H0V120" fill="none" stroke="#1f1f24" strokeWidth="1" />
    </pattern>
    <pattern id="th-hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <line x1="0" y1="0" x2="0" y2="6" stroke="rgba(200,202,208,0.28)" strokeWidth="1.5" />
    </pattern>
    <pattern id="th-hatch-red" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <line x1="0" y1="0" x2="0" y2="6" stroke="rgba(229,36,59,0.45)" strokeWidth="1.5" />
    </pattern>
    <filter id="th-glow" x="-50%" y="-50%" width="200%" height="200%">
      <feGaussianBlur stdDeviation="4" result="b" />
      <feMerge>
        <feMergeNode in="b" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
    <filter id="th-glow-strong" x="-80%" y="-80%" width="260%" height="260%">
      <feGaussianBlur stdDeviation="10" result="b" />
      <feMerge>
        <feMergeNode in="b" />
        <feMergeNode in="b" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
    <radialGradient id="th-vignette" cx="50%" cy="45%" r="70%">
      <stop offset="60%" stopColor="#000" stopOpacity="0" />
      <stop offset="100%" stopColor="#000" stopOpacity="0.65" />
    </radialGradient>
    <radialGradient id="th-spot" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stopColor={C.crimson} stopOpacity="0.22" />
      <stop offset="100%" stopColor={C.crimson} stopOpacity="0" />
    </radialGradient>
    <linearGradient id="th-scan" x1="0" x2="1" y1="0" y2="0">
      <stop offset="0%" stopColor={C.crimson} stopOpacity="0" />
      <stop offset="85%" stopColor={C.crimson} stopOpacity="0.35" />
      <stop offset="100%" stopColor="#fff" stopOpacity="0.9" />
    </linearGradient>
    <linearGradient id="th-card" x1="0" x2="0" y1="0" y2="1">
      <stop offset="0%" stopColor="#1a1a1e" />
      <stop offset="100%" stopColor="#111113" />
    </linearGradient>
  </defs>
);

const Caption: React.FC<{ x: number; y: number; children: React.ReactNode; anchor?: 'start' | 'middle' | 'end'; fill?: string; opacity?: number; size?: number }> = ({
  x,
  y,
  children,
  anchor = 'start',
  fill = C.subtle,
  opacity = 1,
  size = 11,
}) => (
  <text x={x} y={y} textAnchor={anchor} fontFamily={SANS} fontSize={size + 1.5} fill={fill} opacity={opacity}>
    {children}
  </text>
);

/* ------------------------------------------------------------------ 1 · seal */

export const SealScene: React.FC<SceneProps> = ({ model, p }) => {
  const cards = model.exhibits;
  const cols = cards.length <= 4 ? 2 : 3;
  const cardW = cols === 2 ? 420 : 320;
  const cardH = 156;
  const gapX = 36;
  const gapY = 34;
  const rows = Math.ceil(cards.length / cols);
  const gridW = cols * cardW + (cols - 1) * gapX;
  const x0 = (STAGE_W - gridW) / 2;
  const y0 = 96 + (rows === 1 ? 70 : 0);
  const frame = Math.floor(p * 420);
  const chainP = seg(p, 0.7, 0.95);

  if (cards.length === 0) {
    return (
      <g opacity={seg(p, 0, 0.15)}>
        <text x={600} y={320} textAnchor="middle" fontFamily={SERIF} fontWeight={600} fontSize={34} fill={C.fg}>
          No exhibits ingested yet.
        </text>
        <Caption x={600} y={356} anchor="middle">
          yukti evidence ingest &lt;file&gt; --case {model.caseId}
        </Caption>
      </g>
    );
  }

  return (
    <g>
      {cards.map((card, i) => {
        const col = i % cols;
        const row = Math.floor(i / cols);
        const x = x0 + col * (cardW + gapX);
        const y = y0 + row * (cardH + gapY);
        const a = 0.03 + i * 0.06;
        const enter = easeOutExpo(seg(p, a, a + 0.16));
        const decode = seg(p, a + 0.1, a + 0.36);
        const sealed = seg(p, a + 0.36, a + 0.44);
        const hash = card.sha256.slice(0, 32).padEnd(32, '·');
        const resolved = Math.floor(decode * 32);
        const shown = hash
          .split('')
          .map((ch, k) => (k < resolved ? ch : decode > 0 ? hexNoise(k + i * 40, frame) : '·'))
          .join('');
        const scanX = lerp(0, cardW, decode);
        const glow = decode > 0 && decode < 1;
        return (
          <g key={card.id} transform={`translate(${x} ${y + (1 - enter) * 40})`} opacity={enter}>
            <rect width={cardW} height={cardH} fill="url(#th-card)" stroke={sealed > 0.5 ? 'rgba(79,179,125,0.55)' : C.line} />
            <rect width={3} height={cardH} fill={sealed > 0.5 ? C.ok : C.crimson} opacity={0.9} />
            <Caption x={18} y={26} fill={C.crimson}>
              {card.type.slice(0, cols === 2 ? 30 : 17)}
            </Caption>
            {sealed < 0.5 && (
              <Caption x={cardW - 16} y={26} anchor="end">
                {card.id.length > 14 ? `${card.id.slice(0, 8)}…` : card.id}
              </Caption>
            )}
            <text x={18} y={60} fontFamily={DISPLAY} style={displayStyle} fontWeight={600} fontSize={19} fill={C.bright}>
              {card.name.length > (cols === 2 ? 36 : 25) ? `${card.name.slice(0, cols === 2 ? 35 : 24)}…` : card.name}
            </text>
            <Caption x={18} y={80} fill={C.muted}>
              {card.source.slice(0, cols === 2 ? 44 : 34)}
            </Caption>
            <line x1={18} x2={cardW - 18} y1={98} y2={98} stroke={C.line} />
            <Caption x={18} y={120} fill={C.subtle} size={10}>
              SHA-256
            </Caption>
            <text x={80} y={120} fontFamily={MONO} fontSize={cols === 2 ? 12.5 : 10.5} fill={sealed >= 1 ? C.ok : decode >= 1 ? C.fg : C.muted} letterSpacing="0.04em">
              {shown}
            </text>
            <Caption x={18} y={142} size={10}>
              {card.provenance[0]!.toUpperCase() + card.provenance.slice(1)} · read-only original
            </Caption>
            {glow && (
              <g>
                <rect x={Math.max(scanX - 80, 0)} y={0} width={Math.min(80, scanX)} height={cardH} fill="url(#th-scan)" opacity={0.5} />
                <line x1={scanX} x2={scanX} y1={0} y2={cardH} stroke="#fff" strokeWidth={1.5} filter="url(#th-glow)" />
              </g>
            )}
            {sealed > 0 && (
              <g transform={`translate(${cardW - 62} 24) rotate(-6) scale(${lerp(1.8, 0.82, easeOut(sealed))})`} opacity={sealed}>
                <rect x={-52} y={-15} width={104} height={30} fill="none" stroke={C.ok} strokeWidth={2} />
                <text x={0} y={5} textAnchor="middle" fontFamily={MONO} fontWeight={700} fontSize={13} letterSpacing="0.18em" fill={C.ok}>
                  SEALED ✓
                </text>
              </g>
            )}
          </g>
        );
      })}
      {/* Custody ledger: each entry hashes the one before it. */}
      <g opacity={chainP} transform={`translate(0 ${y0 + rows * (cardH + gapY) + 14})`}>
        <Caption x={600} y={0} anchor="middle" fill={C.muted}>
          Custody ledger · append-only · each entry carries the hash of the one before
        </Caption>
        {cards.map((card, i) => {
          const n = cards.length;
          const span = Math.min(760, n * 150);
          const cx = 600 - span / 2 + (span / Math.max(n - 1, 1)) * i;
          const x = n === 1 ? 600 : cx;
          const next = 600 - span / 2 + (span / Math.max(n - 1, 1)) * (i + 1);
          const shown = seg(chainP, i / n, (i + 1) / n);
          return (
            <g key={card.id} opacity={shown}>
              {i < n - 1 && (
                <line x1={x + 16} x2={lerp(x + 16, next - 16, easeOut(seg(chainP, (i + 0.5) / n, (i + 1.2) / n)))} y1={34} y2={34} stroke={C.ok} strokeWidth={1.5} strokeDasharray="3 4" />
              )}
              <circle cx={x} cy={34} r={13} fill={C.bg} stroke={C.ok} strokeWidth={1.5} />
              <text x={x} y={38} textAnchor="middle" fontFamily={MONO} fontSize={10} fill={C.ok}>
                {String(i + 1).padStart(2, '0')}
              </text>
              <Caption x={x} y={64} anchor="middle" size={9}>
                {card.sha256.slice(0, 6)}…
              </Caption>
            </g>
          );
        })}
      </g>
      {model.exhibitTotal > cards.length && (
        <Caption x={600} y={640} anchor="middle" opacity={chainP}>
          + {model.exhibitTotal - cards.length} more exhibits sealed the same way
        </Caption>
      )}
    </g>
  );
};

/* ------------------------------------------------------------------ 2 · one clock */

function niceStep(span: number): number {
  const steps = [30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 21600];
  return steps.find((s) => span / s <= 8) ?? 21600;
}

/** The axis domain at chapter progress `p`: all raw values first, then zoomed to the corrected ones. */
function clockDomain(model: TheatreModel, p: number): { start: number; end: number } {
  const z = easeInOut(seg(p, 0.52, 0.74));
  return {
    start: lerp(model.clock.start, model.clockNormalised.start, z),
    end: lerp(model.clock.end, model.clockNormalised.end, z),
  };
}

export function clockGeometry(model: TheatreModel, p = 1): {
  x0: number;
  x1: number;
  y0: number;
  laneH: number;
  domain: { start: number; end: number };
  xOf: (s: number) => number;
} {
  const x0 = 250;
  const x1 = 1150;
  const y0 = 120;
  const laneH = Math.min(78, 430 / Math.max(model.lanes.length, 1));
  const domain = clockDomain(model, p);
  const xOf = (s: number): number => x0 + ((s - domain.start) / Math.max(domain.end - domain.start, 1)) * (x1 - x0);
  return { x0, x1, y0, laneH, domain, xOf };
}

/** The sweeping "now" cursor in the last third of the clock chapter, as display-clock seconds. */
export function clockCursor(model: TheatreModel, p: number): number | null {
  const q = seg(p, 0.76, 0.98);
  if (q <= 0) return null;
  return lerp(model.clockNormalised.start, model.clockNormalised.end, easeInOut(q));
}

export const ClockScene: React.FC<SceneProps> = ({ model, p }) => {
  const { x0, x1, y0, laneH, xOf, domain } = clockGeometry(model, p);
  const { start, end } = domain;
  const step = niceStep(end - start);
  const ticks: number[] = [];
  // Ticks outside the plot are clipped by the loop bounds below.
  for (let s = Math.ceil(start / step) * step; s <= end; s += step) ticks.push(s);
  const frame = seg(p, 0, 0.14);
  const appear = seg(p, 0.08, 0.32);
  const glide = easeInOut(seg(p, 0.36, 0.66));
  const windows = easeOut(seg(p, 0.66, 0.82));
  const cursor = clockCursor(model, p);
  const yBottom = y0 + model.lanes.length * laneH;
  const conflictEvents = model.conflict ? model.conflict.eventIndices.map((i) => model.events[i]).filter(Boolean) as TEvent[] : [];

  if (model.events.length === 0) {
    return (
      <g opacity={frame}>
        <text x={600} y={320} textAnchor="middle" fontFamily={SERIF} fontWeight={600} fontSize={34} fill={C.fg}>
          No timed events were extracted.
        </text>
        <Caption x={600} y={356} anchor="middle">
          yukti reconstruction run --case {model.caseId}
        </Caption>
      </g>
    );
  }

  return (
    <g>
      <g opacity={frame}>
        <Caption x={x0} y={64} fill={glide < 0.5 ? C.amber : C.fg}>
          {glide < 0.5 ? 'As recorded · each source on its own clock' : 'Normalised · one clock · IST (UTC+05:30)'}
        </Caption>
        <line x1={x0} x2={x1} y1={y0 - 18} y2={y0 - 18} stroke={C.strong} />
        {ticks.filter((s) => xOf(s) >= x0 - 1 && xOf(s) <= x1 + 1).map((s) => (
          <g key={s}>
            <line x1={xOf(s)} x2={xOf(s)} y1={y0 - 22} y2={yBottom} stroke={C.line} strokeDasharray="2 5" />
            <Caption x={xOf(s)} y={y0 - 28} anchor="middle" size={10}>
              {formatWall(s, false)}
            </Caption>
          </g>
        ))}
        {model.lanes.map((lane, i) => (
          <g key={lane.evidenceId} transform={`translate(0 ${y0 + i * laneH})`}>
            <rect x={40} y={0} width={x1 - 40} height={laneH} fill={i % 2 ? 'rgba(255,255,255,0.012)' : 'transparent'} />
            <line x1={40} x2={x1} y1={laneH} y2={laneH} stroke={C.line} />
            <text x={52} y={laneH / 2 - 2} fontFamily={DISPLAY} style={displayStyle} fontWeight={600} fontSize={13} fill={C.fg}>
              {lane.label}
            </text>
            <Caption x={52} y={laneH / 2 + 15} size={9.5}>
              {lane.type.slice(0, 24)}
            </Caption>
          </g>
        ))}
      </g>

      {model.events.map((e) => {
        const a = 0.08 + (e.index / Math.max(model.events.length, 1)) * 0.2;
        const show = easeOut(seg(p, a, a + 0.08)) * appear ** 0.2;
        const cy = y0 + e.lane * laneH + laneH / 2;
        const xRaw = xOf(e.rawWall);
        const x = lerp(xRaw, xOf(e.wall), glide);
        const shift = e.wall - e.rawWall;
        const moved = Math.abs(shift) > 30;
        const lit = cursor !== null && cursor >= e.wall;
        const fill = e.flagged ? C.crimson : glide > 0.5 ? C.fg : C.amber;
        const wx0 = xOf(e.windowStart);
        const wx1 = xOf(e.windowEnd);
        const ww = Math.max(wx1 - wx0, 0);
        return (
          <g key={e.id} opacity={show}>
            {moved && glide > 0 && glide < 1 && (
              <line x1={xRaw} x2={x} y1={cy} y2={cy} stroke={C.amber} strokeDasharray="3 4" opacity={0.7} />
            )}
            {moved && glide < 0.15 && <circle cx={xRaw} cy={cy} r={16} fill="none" stroke={C.amber} opacity={0.25} />}
            {ww > 2 && windows > 0 && (
              <rect
                x={lerp((wx0 + wx1) / 2, wx0, windows)}
                y={cy - 9}
                width={ww * windows}
                height={18}
                fill={e.flagged ? 'url(#th-hatch-red)' : 'url(#th-hatch)'}
                stroke={e.flagged ? 'rgba(229,36,59,0.6)' : 'rgba(200,202,208,0.35)'}
              />
            )}
            {lit && <circle cx={x} cy={cy} r={14} fill={e.flagged ? C.crimson : C.fg} opacity={0.18} filter="url(#th-glow-strong)" />}
            <rect
              x={x - 7}
              y={cy - 7}
              width={14}
              height={14}
              transform={`rotate(45 ${x} ${cy})`}
              fill={glide > 0.5 || e.flagged ? fill : C.bg}
              stroke={fill}
              strokeWidth={1.6}
              filter={e.flagged ? 'url(#th-glow)' : undefined}
            />
            <Caption x={x} y={cy - 16} anchor="middle" fill={glide < 0.5 ? C.amber : e.flagged ? C.crimson : C.muted} size={10}>
              {glide < 0.5 ? formatWall(e.rawWall) : formatWall(e.wall)}
            </Caption>
            {moved && glide >= 0.98 && (
              <Caption x={x} y={cy + 26} anchor="middle" fill={C.amber} size={9.5}>
                {formatDuration(shift)} corrected
              </Caption>
            )}
          </g>
        );
      })}

      {conflictEvents.length === 2 && seg(p, 0.84, 0.94) > 0 && (() => {
        const [ea, eb] = conflictEvents as [TEvent, TEvent];
        const xa = xOf(ea.wall);
        const xb = xOf(eb.wall);
        const ya = y0 + ea.lane * laneH + laneH / 2;
        const yb = y0 + eb.lane * laneH + laneH / 2;
        const q = easeOut(seg(p, 0.84, 0.95));
        const bx = Math.max(xa, xb) + 26;
        return (
          <g opacity={q}>
            <path d={`M${xa} ${ya} C ${bx} ${ya}, ${bx} ${yb}, ${xb} ${yb}`} fill="none" stroke={C.crimson} strokeWidth={1.6} strokeDasharray="4 4" filter="url(#th-glow)" />
            {model.conflict?.gapSeconds !== null && model.conflict?.gapSeconds !== undefined && (
              <g transform={`translate(${Math.max(Math.min(xa, xb) - 190, x0)} ${(ya + yb) / 2})`}>
                <rect x={0} y={-14} width={150} height={28} fill="rgba(229,36,59,0.12)" stroke={C.crimson} />
                <text x={75} y={5} textAnchor="middle" fontFamily={MONO} fontSize={12} fontWeight={700} fill={C.crimson}>
                  Δ {formatDuration(model.conflict.gapSeconds)}
                </text>
              </g>
            )}
          </g>
        );
      })()}

      {cursor !== null && (
        <g>
          <line x1={xOf(cursor)} x2={xOf(cursor)} y1={y0 - 18} y2={yBottom} stroke={C.crimson} strokeWidth={1.2} opacity={0.85} />
          <rect x={xOf(cursor) - 34} y={y0 - 50} width={68} height={18} fill={C.crimson} />
          <text x={xOf(cursor)} y={y0 - 37} textAnchor="middle" fontFamily={MONO} fontSize={10.5} fontWeight={700} fill="#fff">
            {formatWall(cursor)}
          </text>
        </g>
      )}

      <g opacity={seg(p, 0.86, 1)} transform={`translate(40 ${Math.min(yBottom + 46, 630)})`}>
        <rect x={0} y={-8} width={12} height={12} transform="rotate(45 6 -2)" fill="none" stroke={C.amber} />
        <Caption x={22} y={0} fill={C.muted}>Raw recorded value</Caption>
        <rect x={210} y={-10} width={26} height={14} fill="url(#th-hatch)" stroke="rgba(200,202,208,0.35)" />
        <Caption x={244} y={0} fill={C.muted}>Uncertainty window</Caption>
        <rect x={430} y={-8} width={12} height={12} transform="rotate(45 436 -2)" fill={C.crimson} />
        <Caption x={452} y={0} fill={C.muted}>In a contradiction lead</Caption>
        <Caption x={1110} y={0} anchor="end" fill={C.subtle}>
          {model.stats.uncertain} of {model.stats.events} cannot be strictly ordered
        </Caption>
      </g>
    </g>
  );
};

/* ------------------------------------------------------------------ 3 · places */

const MAP = { x: 50, y: 70, w: 800, h: 520 };

function pointAlong(points: { x: number; y: number }[], t: number): { x: number; y: number; length: number; total: number } {
  const lengths = points.slice(1).map((pt, i) => Math.hypot(pt.x - points[i]!.x, pt.y - points[i]!.y));
  const total = lengths.reduce((a, b) => a + b, 0);
  let d = total * clamp(t);
  for (let i = 0; i < lengths.length; i += 1) {
    if (d <= lengths[i]! || i === lengths.length - 1) {
      const f = lengths[i]! === 0 ? 0 : clamp(d / lengths[i]!);
      const a = points[i]!;
      const b = points[i + 1]!;
      return { x: lerp(a.x, b.x, f), y: lerp(a.y, b.y, f), length: total * clamp(t), total };
    }
    d -= lengths[i]!;
  }
  const last = points[points.length - 1] ?? { x: 0, y: 0 };
  return { ...last, length: total, total };
}

export const PlaceScene: React.FC<SceneProps> = ({ model, p, t }) => {
  const frame = seg(p, 0, 0.14);
  if (model.places.length === 0) {
    return (
      <g opacity={frame}>
        <rect x={MAP.x} y={MAP.y} width={1100} height={MAP.h} fill="url(#th-grid-major)" opacity={0.6} />
        <text x={600} y={320} textAnchor="middle" fontFamily={SERIF} fontWeight={600} fontSize={34} fill={C.fg}>
          No exhibit placed an event on a map.
        </text>
        <Caption x={600} y={356} anchor="middle">
          YUKTI never invents a location: events without one stay off the map
        </Caption>
      </g>
    );
  }
  const byId = new Map(model.places.map((pl) => [pl.id, pl]));
  const pts = model.route.map((id) => byId.get(id)).filter(Boolean).map((pl) => ({ x: MAP.x + pl!.x, y: MAP.y + pl!.y }));
  const travel = easeInOut(seg(p, 0.2, 0.78));
  const head = pts.length > 1 ? pointAlong(pts, travel) : null;
  const pathD = pts.map((pt, i) => `${i === 0 ? 'M' : 'L'}${pt.x} ${pt.y}`).join(' ');
  const reachedStops = pts.length > 1 ? pts.map((_, i) => (i === 0 ? 0 : pointAlong(pts.slice(0, i + 1), 1).total)) : [0];
  const transit = model.conflict?.transit ?? null;
  const conflictPlaces = model.conflict
    ? model.conflict.eventIndices.map((i) => model.events[i]?.placeId).filter((id): id is string => Boolean(id))
    : [];

  return (
    <g>
      <g opacity={frame}>
        <rect x={MAP.x} y={MAP.y} width={MAP.w} height={MAP.h} fill="url(#th-grid-major)" stroke={C.line} />
        <Caption x={MAP.x + 12} y={MAP.y + 20}>Schematic plot · relative positions · not a basemap</Caption>
        {/* compass */}
        <g transform={`translate(${MAP.x + MAP.w - 40} ${MAP.y + MAP.h - 40})`} opacity={0.7}>
          <circle r={18} fill="none" stroke={C.strong} />
          <path d="M0 -14 L4 0 L0 14 L-4 0 Z" fill={C.subtle} />
          <path d="M0 -14 L4 0 L-4 0 Z" fill={C.crimson} />
          <Caption x={0} y={-24} anchor="middle" size={9}>N</Caption>
        </g>
      </g>

      {model.places.map((pl, i) => {
        const crowd = model.places.slice(0, i).filter((o) => Math.hypot(o.x - pl.x, o.y - pl.y) < 60).length;
        const labelDy = crowd * 15;
        const show = easeOut(seg(p, 0.06 + i * 0.03, 0.2 + i * 0.03));
        const x = MAP.x + pl.x;
        const y = MAP.y + pl.y;
        const r = pl.radius || (pl.inferred ? 34 : 0);
        const isConflict = conflictPlaces.includes(pl.id);
        const pulse = 0.5 + 0.5 * Math.sin(t * 2.4 + i);
        return (
          <g key={pl.id} opacity={show}>
            {r > 0 && (
              <>
                <circle cx={x} cy={y} r={r} fill="url(#th-hatch)" stroke="rgba(200,202,208,0.35)" strokeDasharray="4 4" />
                <circle cx={x} cy={y} r={r + 6 * pulse} fill="none" stroke="rgba(200,202,208,0.18)" />
              </>
            )}
            {isConflict && seg(p, 0.7, 0.8) > 0 && (
              <circle cx={x} cy={y} r={22 + 10 * pulse} fill="none" stroke={C.crimson} opacity={seg(p, 0.7, 0.8) * 0.8} filter="url(#th-glow)" />
            )}
            <rect x={x - 6} y={y - 6} width={12} height={12} transform={`rotate(45 ${x} ${y})`} fill={C.bg} stroke={isConflict ? C.crimson : C.fg} strokeWidth={1.6} />
            <text x={x} y={y + 28 + labelDy} textAnchor="middle" fontFamily={DISPLAY} style={displayStyle} fontWeight={600} fontSize={12} fill={C.fg} paintOrder="stroke" stroke={C.bg} strokeWidth={4}>
              {pl.name.length > 24 ? `${pl.name.slice(0, 23)}…` : pl.name}
            </text>
            {pl.inferred && (
              <Caption x={x} y={y + 42 + labelDy} anchor="middle" size={9} fill={C.amber}>
                Inferred · not GPS
              </Caption>
            )}
          </g>
        );
      })}

      {pts.length > 1 && head && (
        <g>
          <path d={pathD} fill="none" stroke={C.strong} strokeWidth={1} strokeDasharray="2 6" opacity={frame} />
          <path
            d={pathD}
            fill="none"
            stroke={C.crimson}
            strokeWidth={2.4}
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeDasharray={`${head.length} ${head.total + 10}`}
            filter="url(#th-glow)"
          />
          {pts.map((pt, i) => {
            const reached = head.length + 0.5 >= reachedStops[i]!;
            return reached ? (
              <g key={i} opacity={1}>
                <circle cx={pt.x + 18} cy={pt.y - 18} r={10} fill={C.crimson} />
                <text x={pt.x + 18} y={pt.y - 14.5} textAnchor="middle" fontFamily={MONO} fontSize={10} fontWeight={700} fill="#fff">
                  {i + 1}
                </text>
              </g>
            ) : null;
          })}
          {travel > 0 && travel < 1 && (
            <g>
              <circle cx={head.x} cy={head.y} r={16} fill={C.crimson} opacity={0.25} filter="url(#th-glow-strong)" />
              <circle cx={head.x} cy={head.y} r={5} fill="#fff" />
            </g>
          )}
        </g>
      )}

      {/* stop list */}
      <g transform="translate(884 84)" opacity={seg(p, 0.12, 0.3)}>
        <Caption x={0} y={0} fill={C.muted}>Route in event order</Caption>
        {model.route.slice(0, 9).map((id, i) => {
          const pl = byId.get(id);
          if (!pl) return null;
          const ev = model.events.find((e) => e.placeId === id);
          const reached = head ? head.length + 0.5 >= (reachedStops[i] ?? 0) : true;
          return (
            <g key={`${id}-${i}`} transform={`translate(0 ${26 + i * 44})`} opacity={reached ? 1 : 0.32}>
              <rect x={0} y={0} width={276} height={36} fill={reached ? 'rgba(229,36,59,0.07)' : 'transparent'} stroke={C.line} />
              <text x={14} y={23} fontFamily={MONO} fontSize={12} fontWeight={700} fill={reached ? C.crimson : C.subtle}>
                {String(i + 1).padStart(2, '0')}
              </text>
              <text x={44} y={16} fontFamily={DISPLAY} style={displayStyle} fontWeight={600} fontSize={12} fill={C.fg}>
                {pl.name.length > 26 ? `${pl.name.slice(0, 25)}…` : pl.name}
              </text>
              <Caption x={44} y={29} size={9.5}>
                {ev ? `${formatWall(ev.wall)} IST` : '—'}
              </Caption>
            </g>
          );
        })}
      </g>

      {transit && seg(p, 0.72, 0.84) > 0 && (
        <g transform="translate(884 520)" opacity={easeOut(seg(p, 0.72, 0.86))}>
          <rect x={0} y={0} width={276} height={84} fill="rgba(229,36,59,0.1)" stroke={C.crimson} />
          <Caption x={14} y={20} fill={C.crimson}>Movement check</Caption>
          <text x={14} y={48} fontFamily={MONO} fontSize={15} fontWeight={700} fill={C.bright}>
            {transit.distance_km.toFixed(1)} km in ≤ {Math.round(transit.max_elapsed_seconds)} s
          </text>
          <text x={14} y={70} fontFamily={MONO} fontSize={12} fill={C.crimson}>
            needs ≥ {transit.required_speed_kmh.toFixed(0)} km/h (ref {transit.threshold_kmh.toFixed(0)})
          </text>
        </g>
      )}
    </g>
  );
};

/* ------------------------------------------------------------------ 4 · links */

const GRAPH = { x: 220, y: 120 };

const NodeShape: React.FC<{ node: TNode; r: number; stroke: string; fill: string }> = ({ node, r, stroke, fill }) => {
  switch (node.kind) {
    case 'person':
      return <circle r={r} fill={fill} stroke={stroke} strokeWidth={1.8} />;
    case 'location':
      return <rect x={-r} y={-r} width={r * 2} height={r * 2} transform="rotate(45)" fill={fill} stroke={stroke} strokeWidth={1.8} />;
    case 'event':
      return <rect x={-r * 0.7} y={-r * 0.7} width={r * 1.4} height={r * 1.4} transform="rotate(45)" fill={fill} stroke={stroke} strokeWidth={1.4} strokeDasharray="0" />;
    case 'vehicle':
      return <rect x={-r * 1.3} y={-r * 0.75} width={r * 2.6} height={r * 1.5} rx={3} fill={fill} stroke={stroke} strokeWidth={1.8} />;
    case 'evidence':
      return <rect x={-r * 0.8} y={-r} width={r * 1.6} height={r * 2} fill={fill} stroke={stroke} strokeWidth={1.4} />;
    default:
      return <rect x={-r} y={-r} width={r * 2} height={r * 2} rx={4} fill={fill} stroke={stroke} strokeWidth={1.8} />;
  }
};

export const LinkScene: React.FC<SceneProps> = ({ model, p, t, bare = false }) => {
  const nodes = model.nodes;
  if (nodes.length === 0) {
    return (
      <g opacity={seg(p, 0, 0.15)}>
        <text x={600} y={320} textAnchor="middle" fontFamily={SERIF} fontWeight={600} fontSize={34} fill={C.fg}>
          No entities have been linked yet.
        </text>
      </g>
    );
  }
  const order = new Map(nodes.map((n, i) => [n.id, i]));
  const pos = new Map(nodes.map((n) => [n.id, { x: GRAPH.x + n.x, y: GRAPH.y + n.y }]));
  const nodeAt = (id: string): number => 0.04 + ((order.get(id) ?? 0) / Math.max(nodes.length, 1)) * 0.32;
  const zoom = lerp(0.96, 1.03, easeInOut(p));
  const flow = seg(p, 0.55, 0.62);

  return (
    <g transform={`translate(600 360) scale(${zoom}) translate(-600 -360)`}>
      <ellipse cx={600} cy={340} rx={440} ry={260} fill="url(#th-spot)" opacity={0.5 * seg(p, 0.6, 1)} />
      {model.edges.map((edge, i) => {
        const a = pos.get(edge.from);
        const b = pos.get(edge.to);
        if (!a || !b) return null;
        const start = Math.max(nodeAt(edge.from), nodeAt(edge.to)) + 0.06 + (i % 5) * 0.01;
        const draw = easeOut(seg(p, start, start + 0.16));
        if (draw <= 0) return null;
        const len = Math.hypot(b.x - a.x, b.y - a.y);
        const color = edge.contradiction ? C.crimson : edge.observed ? C.fg : C.silver;
        const k = ((t * (edge.contradiction ? 0.55 : 0.35) + i * 0.37) % 1 + 1) % 1;
        return (
          <g key={edge.id}>
            <line
              x1={a.x}
              y1={a.y}
              x2={lerp(a.x, b.x, draw)}
              y2={lerp(a.y, b.y, draw)}
              stroke={color}
              strokeWidth={edge.contradiction ? 2.2 : 1.3}
              strokeDasharray={edge.observed && !edge.contradiction ? undefined : edge.contradiction ? '7 5' : '4 5'}
              opacity={edge.contradiction ? 1 : 0.7}
              filter={edge.contradiction ? 'url(#th-glow)' : undefined}
            />
            {flow > 0 && draw >= 1 && len > 30 && (
              <circle cx={lerp(a.x, b.x, k)} cy={lerp(a.y, b.y, k)} r={edge.contradiction ? 3.6 : 2.4} fill={edge.contradiction ? C.crimson : '#fff'} opacity={flow * (edge.observed ? 0.95 : 0.6)} filter="url(#th-glow)" />
            )}
            {edge.contradiction && draw >= 1 && (
              <Caption x={(a.x + b.x) / 2} y={(a.y + b.y) / 2 - 10} anchor="middle" fill={C.crimson} size={10}>
                {edge.label}
              </Caption>
            )}
          </g>
        );
      })}
      {nodes.map((node) => {
        const at = nodeAt(node.id);
        const show = easeOutExpo(seg(p, at, at + 0.1));
        if (show <= 0) return null;
        const { x, y } = pos.get(node.id)!;
        const r = 9 + Math.min(node.degree, 6) * 1.6;
        const hub = node.degree >= 3;
        const touchesConflict = model.edges.some((e) => e.contradiction && (e.from === node.id || e.to === node.id));
        const stroke = touchesConflict ? C.crimson : node.kind === 'event' ? C.steel : C.fg;
        return (
          <g key={node.id} transform={`translate(${x} ${y}) scale(${show})`}>
            {hub && <circle r={r + 12} fill={touchesConflict ? C.crimson : '#fff'} opacity={0.08 + 0.05 * Math.sin(t * 2 + x)} />}
            <NodeShape node={node} r={r} stroke={stroke} fill={C.bg} />
            {node.kind === 'location' && <rect x={-3} y={-3} width={6} height={6} fill={C.amber} />}
            <text y={r + 18} textAnchor="middle" fontFamily={SANS} fontWeight={500} fontSize={12} fill={C.fg} paintOrder="stroke" stroke={C.bg} strokeWidth={4} opacity={seg(p, at + 0.06, at + 0.14)}>
              {node.label}
            </text>
            <text y={r + 30} textAnchor="middle" fontFamily={SANS} fontSize={9.5} fill={C.subtle} opacity={seg(p, at + 0.06, at + 0.14)}>
              {node.kind}
            </text>
          </g>
        );
      })}
      <g transform="translate(40 96)" opacity={bare ? 0 : seg(p, 0.1, 0.3)}>
        <Caption x={0} y={0} fill={C.muted}>How to read a link</Caption>
        <line x1={0} x2={40} y1={24} y2={24} stroke={C.fg} strokeWidth={1.4} />
        <Caption x={50} y={28} fill={C.fg}>Observed</Caption>
        <line x1={0} x2={40} y1={48} y2={48} stroke={C.silver} strokeWidth={1.3} strokeDasharray="4 5" />
        <Caption x={50} y={52} fill={C.silver}>Inferred</Caption>
        <line x1={0} x2={40} y1={72} y2={72} stroke={C.crimson} strokeWidth={2.2} strokeDasharray="7 5" />
        <Caption x={50} y={76} fill={C.crimson}>Contradicts</Caption>
      </g>
    </g>
  );
};

/* ------------------------------------------------------------------ 5 · conflict */

const Slab: React.FC<{ x: number; y: number; label: string; value: string; evidence: string; enter: number; from: number; tone: string }> = ({
  x,
  y,
  label,
  value,
  evidence,
  enter,
  from,
  tone,
}) => {
  const lines = wrap(value, 22, 2);
  return (
    <g transform={`translate(${x + (1 - enter) * from} ${y})`} opacity={enter}>
      <rect width={400} height={170} fill="url(#th-card)" stroke={C.strong} />
      <rect width={400} height={3} fill={tone} />
      <Caption x={20} y={32} fill={C.muted}>{label.slice(0, 44)}</Caption>
      {lines.map((line, i) => (
        <text key={i} x={20} y={86 + i * 40} fontFamily={MONO} fontSize={lines.length > 1 ? 26 : 34} fontWeight={700} fill={C.bright}>
          {line}
        </text>
      ))}
      <Caption x={20} y={154} size={10}>
        Source {evidence.length > 18 ? `${evidence.slice(0, 8)}…` : evidence}
      </Caption>
    </g>
  );
};

export const ConflictScene: React.FC<SceneProps> = ({ model, p }) => {
  const c = model.conflict;
  if (!c) {
    const q = easeOut(seg(p, 0.05, 0.3));
    return (
      <g opacity={q}>
        <circle cx={600} cy={280} r={54} fill="none" stroke={C.ok} strokeWidth={2} />
        <path d="M576 282 l16 16 l34 -36" fill="none" stroke={C.ok} strokeWidth={4} strokeDasharray="90" strokeDashoffset={90 * (1 - seg(p, 0.15, 0.4))} />
        <text x={600} y={390} textAnchor="middle" fontFamily={SERIF} fontWeight={600} fontSize={34} fill={C.fg}>
          No contradiction leads in this reconstruction.
        </text>
        <Caption x={600} y={424} anchor="middle">
          That is a finding about these exhibits only · gaps are listed in the report
        </Caption>
      </g>
    );
  }
  const title = easeOut(seg(p, 0, 0.12));
  const left = easeOutExpo(seg(p, 0.06, 0.24));
  const right = easeOutExpo(seg(p, 0.12, 0.3));
  const disc = easeOutExpo(seg(p, 0.3, 0.4));
  const rings = [0, 1, 2].map((k) => seg(p, 0.32 + k * 0.08, 0.52 + k * 0.08));
  const typed = seg(p, 0.6, 0.86);
  const discrepancy = wrap(c.discrepancy, 92, 2);
  const totalChars = discrepancy.join(' ').length;
  let remaining = Math.floor(totalChars * typed);
  const action = seg(p, 0.88, 0.98);

  return (
    <g>
      <g opacity={title}>
        <Caption x={600} y={66} anchor="middle" fill={C.crimson}>Contradiction lead · {c.id.length > 12 ? c.id.slice(0, 8) : c.id}</Caption>
        <text x={600} y={104} textAnchor="middle" fontFamily={DISPLAY} style={displayStyle} fontWeight={700} fontSize={28} fill={C.bright}>
          {c.title.length > 56 ? `${c.title.slice(0, 55)}…` : c.title}
        </text>
      </g>
      <Slab x={110} y={150} label={c.a.label} value={c.a.value} evidence={c.a.evidenceId} enter={left} from={-160} tone={C.steel} />
      <Slab x={690} y={150} label={c.b.label} value={c.b.value} evidence={c.b.evidenceId} enter={right} from={160} tone={C.steel} />
      {rings.map((r, k) =>
        r > 0 && r < 1 ? <circle key={k} cx={600} cy={235} r={44 + r * 120} fill="none" stroke={C.crimson} strokeWidth={2} opacity={(1 - r) * 0.7} /> : null,
      )}
      <g transform={`translate(600 235) scale(${disc})`} opacity={disc}>
        <circle r={44} fill={C.crimson} filter="url(#th-glow-strong)" />
        <text y={14} textAnchor="middle" fontFamily={DISPLAY} fontWeight={800} fontSize={42} fill="#fff">
          ≠
        </text>
      </g>

      {c.transit ? (
        (() => {
          const g = easeInOut(seg(p, 0.42, 0.7));
          const max = Math.max(c.transit.required_speed_kmh * 1.25, c.transit.threshold_kmh * 1.6);
          const angle = (v: number): number => Math.PI + (clamp(v / max) * Math.PI);
          const cx = 600;
          const cy = 470;
          const R = 100;
          const arc = (from: number, to: number): string => {
            const a0 = angle(from);
            const a1 = angle(to);
            return `M${cx + R * Math.cos(a0)} ${cy + R * Math.sin(a0)} A ${R} ${R} 0 0 1 ${cx + R * Math.cos(a1)} ${cy + R * Math.sin(a1)}`;
          };
          const needle = angle(lerp(0, c.transit.required_speed_kmh, g));
          return (
            <g opacity={seg(p, 0.38, 0.46)}>
              <path d={arc(0, c.transit.threshold_kmh)} fill="none" stroke={C.strong} strokeWidth={10} />
              <path d={arc(c.transit.threshold_kmh, max)} fill="none" stroke="rgba(229,36,59,0.35)" strokeWidth={10} />
              <line x1={cx} y1={cy} x2={cx + (R - 8) * Math.cos(needle)} y2={cy + (R - 8) * Math.sin(needle)} stroke="#fff" strokeWidth={3} strokeLinecap="round" filter="url(#th-glow)" />
              <circle cx={cx} cy={cy} r={6} fill="#fff" />
              <text x={cx} y={cy + 34} textAnchor="middle" fontFamily={MONO} fontSize={22} fontWeight={700} fill={g > 0.95 ? C.crimson : C.bright}>
                {Math.round(lerp(0, c.transit.required_speed_kmh, g))} km/h
              </text>
              <Caption x={cx - R - 20} y={cy + 4} anchor="end">{c.transit.distance_km.toFixed(1)} KM</Caption>
              <Caption x={cx + R + 20} y={cy + 4}>Ref {c.transit.threshold_kmh.toFixed(0)} KM/H</Caption>
            </g>
          );
        })()
      ) : c.gapSeconds !== null ? (
        (() => {
          const g = easeInOut(seg(p, 0.42, 0.66));
          const half = lerp(10, 260, g);
          return (
            <g opacity={seg(p, 0.38, 0.46)}>
              <line x1={600 - half} x2={600 + half} y1={430} y2={430} stroke={C.crimson} strokeWidth={2} />
              <line x1={600 - half} x2={600 - half} y1={414} y2={446} stroke={C.fg} strokeWidth={2} />
              <line x1={600 + half} x2={600 + half} y1={414} y2={446} stroke={C.fg} strokeWidth={2} />
              <text x={600} y={478} textAnchor="middle" fontFamily={MONO} fontSize={26} fontWeight={700} fill={C.crimson}>
                {formatDuration((c.gapSeconds ?? 0) * g)}
              </text>
              <Caption x={600} y={500} anchor="middle">Between two records of the same moment</Caption>
            </g>
          );
        })()
      ) : null}

      <g transform="translate(110 560)">
        {discrepancy.map((line, i) => {
          const visible = line.slice(0, Math.max(0, remaining));
          remaining -= line.length + 1;
          return (
            <text key={i} x={0} y={i * 22} fontFamily={MONO} fontSize={13.5} fill={C.fg}>
              {visible}
              {typed > 0 && typed < 1 && remaining < 0 && remaining > -line.length - 2 ? '▌' : ''}
            </text>
          );
        })}
        <text x={0} y={60} fontFamily={MONO} fontSize={12} fill={C.amber} opacity={action}>
          NEXT ▸ {c.action.length > 110 ? `${c.action.slice(0, 109)}…` : c.action}
        </text>
      </g>
    </g>
  );
};

/* ------------------------------------------------------------------ 6 · brief */

export const BriefScene: React.FC<SceneProps> = ({ model, p }) => {
  const s = model.stats;
  const tiles: { value: number; label: string; tone: string }[] = [
    { value: s.exhibits, label: 'Exhibits sealed (SHA-256)', tone: C.ok },
    { value: s.events, label: 'Events on one clock', tone: C.fg },
    { value: s.places, label: 'Places on the plot', tone: C.fg },
    { value: s.links, label: s.inferredLinks ? `Links · ${s.inferredLinks} inferred` : 'Links with a basis', tone: C.silver },
    { value: s.leads, label: 'Leads for review', tone: C.crimson },
    { value: s.assumptions, label: 'Assumptions labelled', tone: C.amber },
  ];
  const head = easeOut(seg(p, 0, 0.15));
  const principles = [
    'Observed facts are kept apart from inferred links.',
    'Every finding traces back to a hash and a source row.',
    'A contradiction is a lead for a human, never a verdict.',
  ];
  return (
    <g>
      <g opacity={head} transform={`translate(0 ${(1 - head) * 14})`}>
        <Caption x={600} y={64} anchor="middle" fill={C.crimson}>Investigative brief · {model.caseId.length > 14 ? model.caseId.slice(0, 8) : model.caseId}</Caption>
        <text x={600} y={112} textAnchor="middle" fontFamily={SERIF} fontWeight={600} fontSize={38} fill={C.bright}>
          The evidence, reconstructed — with every limit stated.
        </text>
      </g>
      {tiles.map((tile, i) => {
        const col = i % 3;
        const row = Math.floor(i / 3);
        const x = 130 + col * 320;
        const y = 160 + row * 150;
        const a = 0.08 + i * 0.05;
        const enter = easeOutExpo(seg(p, a, a + 0.14));
        const count = Math.round(tile.value * easeOut(seg(p, a + 0.02, a + 0.3)));
        return (
          <g key={tile.label} transform={`translate(${x} ${y + (1 - enter) * 24})`} opacity={enter}>
            <rect width={300} height={128} fill="url(#th-card)" stroke={C.line} />
            <rect width={300} height={2} fill={tile.tone} opacity={0.8} />
            <text x={24} y={78} fontFamily={DISPLAY} style={displayStyle} fontWeight={700} fontSize={58} fill={tile.tone === C.fg ? C.bright : tile.tone}>
              {String(count).padStart(2, '0')}
            </text>
            <Caption x={24} y={108} fill={C.muted}>{tile.label}</Caption>
          </g>
        );
      })}
      {principles.map((line, i) => {
        const q = easeOut(seg(p, 0.45 + i * 0.08, 0.6 + i * 0.08));
        return (
          <g key={line} opacity={q} transform={`translate(${130 + (1 - q) * -20} ${506 + i * 34})`}>
            <rect x={0} y={-12} width={8} height={8} fill={i === 2 ? C.crimson : C.fg} />
            <text x={22} y={-4} fontFamily={DISPLAY} style={displayStyle} fontWeight={500} fontSize={17} fill={C.fg}>
              {line}
            </text>
          </g>
        );
      })}
    </g>
  );
};
