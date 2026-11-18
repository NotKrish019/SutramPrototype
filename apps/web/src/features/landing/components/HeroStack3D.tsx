import React, { useState } from 'react';
import { Cloud, Landmark, Mail, Share2, Smartphone, Video, type LucideIcon } from 'lucide-react';
import { DEMO_CASE } from '../../../fixtures/synthetic/case026';
import type { SourceId } from '../data/siteContent';

/**
 * The YUKTI idea as one dimensional object: evidence sources (floor) → correlation layer →
 * knowledge graph → reconstructed timeline. Pure CSS 3D + SVG; synthetic data.
 */

const PLANE = 320;
const Z = { evidence: 0, correlation: 135, graph: 270, timeline: 380 } as const;
const HUB = { x: 160, y: 160 };

interface StackSource {
  id: SourceId;
  label: string;
  icon: LucideIcon;
  entities: string[];
  events: string[];
}

const SOURCES: StackSource[] = [
  { id: 'mobile', label: 'Mobile', icon: Smartphone, entities: ['ENT-DEV', 'ENT-PA'], events: ['EVT-2'] },
  { id: 'cctv', label: 'CCTV', icon: Video, entities: ['ENT-VEH', 'ENT-LOC'], events: ['EVT-3'] },
  { id: 'email', label: 'Email', icon: Mail, entities: ['ENT-PA'], events: [] },
  { id: 'social', label: 'Social', icon: Share2, entities: ['ENT-PA'], events: [] },
  { id: 'financial', label: 'Finance', icon: Landmark, entities: ['ENT-ACC', 'ENT-PA'], events: ['EVT-1'] },
  { id: 'network', label: 'Network', icon: Cloud, entities: ['ENT-DEV', 'ENT-LOC'], events: ['EVT-4', 'EVT-5'] },
];

const tileCenter = (index: number): { x: number; y: number } => ({ x: 62 + (index % 3) * 98, y: 96 + Math.floor(index / 3) * 128 });

const GRAPH_NODES: Record<string, { x: number; y: number; label: string }> = {
  'ENT-PA': { x: 160, y: 70, label: 'Person A' },
  'ENT-DEV': { x: 70, y: 150, label: 'D-01' },
  'ENT-VEH': { x: 250, y: 150, label: 'V-1' },
  'ENT-LOC': { x: 160, y: 230, label: 'Gate B' },
  'ENT-ACC': { x: 70, y: 260, label: '··8812' },
};

const GRAPH_EDGES: { a: string; b: string; kind: 'observed' | 'supplied' | 'inferred' }[] = [
  { a: 'ENT-PA', b: 'ENT-DEV', kind: 'inferred' },
  { a: 'ENT-PA', b: 'ENT-VEH', kind: 'inferred' },
  { a: 'ENT-DEV', b: 'ENT-LOC', kind: 'observed' },
  { a: 'ENT-VEH', b: 'ENT-LOC', kind: 'observed' },
  { a: 'ENT-ACC', b: 'ENT-PA', kind: 'supplied' },
];

const EDGE_STYLE = {
  observed: { stroke: '#EDEDEF', dash: undefined },
  supplied: { stroke: '#7DB4FF', dash: '2 5' },
  inferred: { stroke: '#FFB224', dash: '7 6' },
} as const;

const FLAGGED = DEMO_CASE.discrepancies[0]?.recordA.evidenceId;

/** A vertical beam between two planes, anchored at (x, y) on the plane below. */
const Beam: React.FC<{ x: number; y: number; from: number; to: number; lit: boolean; delay: number }> = ({ x, y, from, to, lit, delay }) => (
  <span
    aria-hidden="true"
    className={`stack-beam pointer-events-none absolute w-[2px] transition-opacity duration-base ${lit ? 'opacity-100' : 'opacity-[0.12]'}`}
    style={
      {
        left: x - 1,
        top: y,
        height: to - from,
        transformOrigin: 'top center',
        transform: `translateZ(${from}px) rotateX(90deg)`,
        '--beam-delay': `${delay}s`,
      } as React.CSSProperties
    }
  />
);

/** One plane of the stack. Only the evidence floor (z = 0) is interactive; upper planes ignore pointer events. */
const Layer: React.FC<{ z: number; delay: number; className?: string; style?: React.CSSProperties; children: React.ReactNode }> = ({
  z,
  delay,
  className = '',
  style,
  children,
}) => (
  <div className={`stack-layer absolute left-0 top-0 ${z > 0 ? 'pointer-events-none' : ''}`} style={{ width: PLANE, height: PLANE, transform: `translateZ(${z}px)` }}>
    <div className="stack-float h-full w-full" style={{ '--float-delay': `${delay}s` } as React.CSSProperties}>
      <div className={`stack-enter relative h-full w-full ${className}`} style={{ '--enter-delay': `${delay * 0.25}s`, ...style } as React.CSSProperties}>
        {children}
      </div>
    </div>
  </div>
);

const STEPS = [
  { n: '04', label: 'Reconstruction' },
  { n: '03', label: 'Connected intelligence' },
  { n: '02', label: 'Correlation' },
  { n: '01', label: 'Evidence sources' },
];

export const HeroStack3D: React.FC = () => {
  const [active, setActive] = useState<SourceId | null>(null);
  const source = SOURCES.find((s) => s.id === active) ?? null;
  const entityLit = (id: string): boolean => !source || source.entities.includes(id);
  const eventLit = (id: string): boolean => !source || source.events.includes(id);

  return (
    <figure className="relative" aria-label="Evidence sources feed a correlation layer, which produces a knowledge graph and a reconstructed timeline. Synthetic demonstration data.">
      <div className="pointer-events-none absolute right-0 top-0 z-10 hidden flex-col gap-[4.4rem] pt-8 xl:flex" aria-hidden="true">
        {STEPS.map((step) => (
          <span key={step.n} className="flex items-center gap-2 font-mono text-micro uppercase text-fg-subtle">
            <span className="h-px w-6 bg-ink-strong" />
            <span className="text-signal-amber">{step.n}</span> {step.label}
          </span>
        ))}
      </div>

      <div className="stack-scene relative mx-auto h-[25rem] w-full overflow-hidden sm:h-[36rem]">
        <div className="absolute left-1/2 top-[62%] origin-center scale-[0.62] sm:top-[66%] sm:scale-[0.9] xl:-translate-x-[22%]" style={{ width: PLANE, height: PLANE, marginLeft: -PLANE / 2, marginTop: -PLANE / 2 }}>
          <div className="stack-stage relative h-full w-full" style={{ transform: 'rotateX(57deg) rotateZ(-42deg)' }}>
            {/* 01 · Evidence sources */}
            <Layer z={Z.evidence} delay={0} className="border border-ink-strong bg-ink-900/90 shadow-[0_60px_80px_-30px_rgba(0,0,0,0.9)]">
              <div aria-hidden="true" className="pattern-dots absolute inset-0 opacity-40" />
              {SOURCES.map((s, i) => {
                const c = tileCenter(i);
                const isActive = active === s.id;
                return (
                  <button
                    key={s.id}
                    type="button"
                    aria-pressed={isActive}
                    aria-label={`Trace ${s.label} evidence`}
                    onMouseEnter={() => setActive(s.id)}
                    onMouseLeave={() => setActive(null)}
                    onFocus={() => setActive(s.id)}
                    onBlur={() => setActive(null)}
                    onClick={() => setActive((current) => (current === s.id ? null : s.id))}
                    className={`absolute flex h-[76px] w-[76px] flex-col items-center justify-center gap-2 border font-mono text-[10px] uppercase tracking-[0.06em] transition-colors duration-base focus:outline-none ${
                      isActive ? 'border-signal-amber bg-[#2a2112] text-signal-amber' : 'border-ink-strong bg-ink-950 text-fg-muted hover:text-fg'
                    }`}
                    style={{ left: c.x - 38, top: c.y - 38 }}
                  >
                    <s.icon aria-hidden="true" className="h-5 w-5" />
                    {s.label}
                  </button>
                );
              })}
              {SOURCES.map((s, i) => {
                const c = tileCenter(i);
                return <Beam key={s.id} x={c.x} y={c.y} from={Z.evidence} to={Z.correlation} lit={!active || active === s.id} delay={i * 0.35} />;
              })}
            </Layer>

            {/* 02 · Correlation layer */}
            <Layer z={Z.correlation} delay={0.6} className="border border-fg/40 bg-ink-850/70">
              <div aria-hidden="true" className="pattern-grid absolute inset-0 opacity-70" />
              <svg viewBox={`0 0 ${PLANE} ${PLANE}`} className="absolute inset-0 h-full w-full" aria-hidden="true">
                {SOURCES.map((s, i) => {
                  const c = tileCenter(i);
                  const lit = !active || active === s.id;
                  return (
                    <g key={s.id} opacity={lit ? 1 : 0.15} style={{ transition: 'opacity 250ms' }}>
                      <line x1={c.x} y1={c.y} x2={HUB.x} y2={HUB.y} stroke="#FFB224" strokeOpacity="0.7" strokeWidth="1.2" className="flow-line" />
                      <rect x={c.x - 4} y={c.y - 4} width="8" height="8" fill="#FFB224" />
                    </g>
                  );
                })}
                <rect x={HUB.x - 22} y={HUB.y - 22} width="44" height="44" fill="#09090A" stroke="#FFB224" strokeWidth="1.5" />
                <rect x={HUB.x - 6} y={HUB.y - 6} width="12" height="12" fill="#FFB224" />
              </svg>
              <Beam x={HUB.x} y={HUB.y} from={Z.correlation} to={Z.graph} lit delay={0.2} />
            </Layer>

            {/* 03 · Knowledge graph */}
            <Layer z={Z.graph} delay={1.2} className="border border-ink-strong/70 bg-ink-950/40">
              <svg viewBox={`0 0 ${PLANE} ${PLANE}`} className="absolute inset-0 h-full w-full" aria-hidden="true">
                {GRAPH_EDGES.map((edge) => {
                  const a = GRAPH_NODES[edge.a]!;
                  const b = GRAPH_NODES[edge.b]!;
                  const style = EDGE_STYLE[edge.kind];
                  const lit = entityLit(edge.a) && entityLit(edge.b);
                  return (
                    <line key={`${edge.a}-${edge.b}`} x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke={style.stroke} strokeDasharray={style.dash} strokeWidth="1.6" opacity={lit ? 0.95 : 0.15} style={{ transition: 'opacity 250ms' }} />
                  );
                })}
                {Object.entries(GRAPH_NODES).map(([id, node]) => {
                  const lit = entityLit(id);
                  const highlighted = !!source && source.entities.includes(id);
                  return (
                    <g key={id} opacity={lit ? 1 : 0.2} style={{ transition: 'opacity 250ms' }}>
                      <rect x={node.x - 9} y={node.y - 9} width="18" height="18" fill={highlighted ? '#FFB224' : '#09090A'} stroke={highlighted ? '#FFB224' : '#EDEDEF'} strokeWidth="1.6" />
                      <text x={node.x} y={node.y + 26} textAnchor="middle" fontFamily="JetBrains Mono, monospace" fontSize="12" fill="#EDEDEF" className="svg-label">
                        {node.label}
                      </text>
                    </g>
                  );
                })}
              </svg>
              <Beam x={HUB.x} y={GRAPH_NODES['ENT-PA']!.y} from={Z.graph} to={Z.timeline} lit delay={0.8} />
            </Layer>

            {/* 04 · Reconstructed timeline */}
            <Layer z={Z.timeline} delay={1.8} style={{ height: PLANE }}>
              <div className="absolute left-0 right-0 border border-ink-strong bg-ink-900/90" style={{ top: 40, height: 56 }}>
                <svg viewBox="0 0 320 56" className="h-full w-full" aria-hidden="true">
                  <line x1="18" x2="302" y1="28" y2="28" stroke="#36363C" />
                  {DEMO_CASE.events.map((event, i) => {
                    const x = 30 + i * 65;
                    const flagged = event.evidenceId === FLAGGED;
                    const lit = eventLit(event.id);
                    return (
                      <g key={event.id} opacity={lit ? 1 : 0.2} style={{ transition: 'opacity 250ms' }}>
                        <rect x={x - 5} y={23} width="10" height="10" transform={`rotate(45 ${x} 28)`} fill={flagged ? '#FF5A4E' : '#EDEDEF'} />
                        <text x={x} y={50} textAnchor="middle" fontFamily="JetBrains Mono, monospace" fontSize="10" fill="#A3A3AB">
                          {event.time.slice(0, 5)}
                        </text>
                      </g>
                    );
                  })}
                </svg>
              </div>
            </Layer>
          </div>
        </div>
      </div>

      <figcaption className="flex flex-wrap items-center justify-between gap-3 border-t border-ink-line pt-3 font-mono text-micro uppercase text-fg-subtle">
        <span>
          {source ? (
            <span className="text-signal-amber">
              Tracing {source.label} → {source.entities.length} entities · {source.events.length} events
            </span>
          ) : (
            <>
              <span className="hidden sm:inline">Hover</span>
              <span className="sm:hidden">Tap</span> a source to trace it
            </>
          )}
        </span>
        <span>{DEMO_CASE.id} · synthetic data</span>
      </figcaption>
    </figure>
  );
};
