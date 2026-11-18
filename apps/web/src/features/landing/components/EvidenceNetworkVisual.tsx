import React, { useState } from 'react';
import { Cloud, Landmark, Mail, Share2, Smartphone, Video, type LucideIcon } from 'lucide-react';
import { DEMO_CASE, type ProvenanceClass } from '../../../fixtures/synthetic/case026';
import type { SourceId } from '../data/siteContent';
import { Frame, PROVENANCE_STYLES, ProvenanceLegend } from '../../../components/ui/primitives';

interface HeroSource {
  id: SourceId;
  label: string;
  icon: LucideIcon;
  /** Hero entity ids this source contributes to. */
  entities: string[];
  /** Event ids this source contributes to. */
  events: string[];
}

const HERO_SOURCES: HeroSource[] = [
  { id: 'mobile', label: 'Mobile device', icon: Smartphone, entities: ['ENT-DEV', 'ENT-PA'], events: ['EVT-2'] },
  { id: 'cctv', label: 'CCTV / video', icon: Video, entities: ['ENT-VEH', 'ENT-LOC'], events: ['EVT-3'] },
  { id: 'email', label: 'Email', icon: Mail, entities: ['ENT-PA'], events: [] },
  { id: 'social', label: 'Social media', icon: Share2, entities: ['ENT-PA'], events: [] },
  { id: 'financial', label: 'Financial', icon: Landmark, entities: ['ENT-ACC', 'ENT-PA'], events: ['EVT-1'] },
  { id: 'network', label: 'Cloud & network', icon: Cloud, entities: ['ENT-DEV', 'ENT-LOC'], events: ['EVT-4', 'EVT-5'] },
];

/** Hero-specific layout for a subset of the demo entities (600×420 viewBox). */
const HERO_NODES: Record<string, { x: number; y: number; label: string }> = {
  'ENT-PA': { x: 474, y: 52, label: 'Person A' },
  'ENT-DEV': { x: 400, y: 128, label: 'Device D-01' },
  'ENT-VEH': { x: 548, y: 124, label: 'Vehicle V-1' },
  'ENT-LOC': { x: 474, y: 198, label: 'Gate B' },
  'ENT-ACC': { x: 556, y: 236, label: 'Card ··8812' },
};

const HERO_EDGES: { from: string; to: string; provenance: ProvenanceClass }[] = [
  { from: 'ENT-PA', to: 'ENT-DEV', provenance: 'inferred' },
  { from: 'ENT-DEV', to: 'ENT-LOC', provenance: 'observed' },
  { from: 'ENT-VEH', to: 'ENT-LOC', provenance: 'observed' },
  { from: 'ENT-PA', to: 'ENT-VEH', provenance: 'inferred' },
  { from: 'ENT-ACC', to: 'ENT-PA', provenance: 'supplied' },
];

const SOURCE_Y = (index: number): number => 24 + index * 50;
const PORT_Y = (index: number): number => 118 + index * 21;
const LAYER = { x: 232, y: 96, w: 118, h: 150 };
const EVENT_X = (index: number): number => 214 + index * 89;
const TIMELINE_Y = 352;
const FLAGGED_EVENT = DEMO_CASE.discrepancies[0]?.recordA.evidenceId;

function sourcePath(index: number): string {
  const sy = SOURCE_Y(index) + 18;
  const py = PORT_Y(index);
  return `M162 ${sy} C 200 ${sy}, 194 ${py}, ${LAYER.x} ${py}`;
}

interface DiagramProps {
  active: SourceId | null;
  onActivate: (id: SourceId | null) => void;
  /** Mobile variant omits the source column (rendered as HTML chips instead). */
  compact?: boolean;
}

const NetworkDiagram: React.FC<DiagramProps> = ({ active, onActivate, compact = false }) => {
  const activeSource = HERO_SOURCES.find((source) => source.id === active) ?? null;
  const isEntityLit = (id: string): boolean => !activeSource || activeSource.entities.includes(id);
  const isEventLit = (id: string): boolean => !activeSource || activeSource.events.includes(id);
  const layerRight = LAYER.x + LAYER.w;
  const layerMidY = LAYER.y + LAYER.h / 2;

  return (
    <svg
      viewBox={compact ? '196 0 404 420' : '0 0 600 420'}
      className="h-auto w-full"
      role="img"
      aria-labelledby={compact ? 'hero-diagram-title-compact' : 'hero-diagram-title'}
    >
      <title id={compact ? 'hero-diagram-title-compact' : 'hero-diagram-title'}>
        Six evidence source types feed a correlation layer that produces connected entities and an ordered event
        sequence. Synthetic demonstration data.
      </title>

      {/* Source channels */}
      {HERO_SOURCES.map((source, index) => {
        const lit = !active || active === source.id;
        const d = compact ? `M196 ${PORT_Y(index)} L ${LAYER.x} ${PORT_Y(index)}` : sourcePath(index);
        return (
          <g key={source.id}>
            <path d={d} fill="none" stroke="#36363C" strokeWidth="1" />
            <path
              d={d}
              fill="none"
              stroke={active === source.id ? '#FFB224' : '#EDEDEF'}
              strokeOpacity={lit ? (active === source.id ? 1 : 0.55) : 0.08}
              strokeWidth="1.25"
              className="flow-line"
              style={{ animationDelay: `${index * -0.45}s`, transition: 'stroke-opacity 250ms' }}
            />
            {!compact && (
              <g
                role="button"
                tabIndex={0}
                aria-pressed={active === source.id}
                aria-label={`Trace ${source.label} evidence`}
                onMouseEnter={() => onActivate(source.id)}
                onMouseLeave={() => onActivate(null)}
                onFocus={() => onActivate(source.id)}
                onBlur={() => onActivate(null)}
                className="cursor-pointer focus:outline-none [&:focus-visible>rect:first-child]:stroke-signal-amber"
              >
                <rect
                  x="12"
                  y={SOURCE_Y(index)}
                  width="150"
                  height="36"
                  fill={active === source.id ? '#1A1A1D' : '#0F0F11'}
                  stroke={active === source.id ? '#FFB224' : '#36363C'}
                  style={{ transition: 'stroke 150ms, fill 150ms' }}
                />
                <source.icon
                  x={24}
                  y={SOURCE_Y(index) + 11}
                  width={14}
                  height={14}
                  color={active === source.id ? '#FFB224' : '#A3A3AB'}
                  strokeWidth={1.75}
                  aria-hidden="true"
                />
                <text
                  x="46"
                  y={SOURCE_Y(index) + 22}
                  fill={lit ? '#EDEDEF' : '#6E6E77'}
                  fontSize="11"
                  fontFamily="JetBrains Mono, ui-monospace, monospace"
                  style={{ textTransform: 'uppercase', letterSpacing: '0.04em' }}
                >
                  {source.label}
                </text>
                <rect x="158" y={SOURCE_Y(index) + 15} width="6" height="6" fill={lit ? '#EDEDEF' : '#36363C'} />
              </g>
            )}
          </g>
        );
      })}

      {/* Correlation layer */}
      <g>
        <rect x={LAYER.x} y={LAYER.y} width={LAYER.w} height={LAYER.h} fill="#141416" stroke="#EDEDEF" strokeOpacity="0.7" />
        <rect x={LAYER.x - 4} y={LAYER.y - 4} width="8" height="8" fill="#FFB224" />
        <text x={LAYER.x + 12} y={LAYER.y + 22} fill="#FFB224" fontSize="9.5" fontFamily="JetBrains Mono, monospace" letterSpacing="0.08em">
          YUKTI
        </text>
        <text x={LAYER.x + 12} y={LAYER.y + 36} fill="#A3A3AB" fontSize="9.5" fontFamily="JetBrains Mono, monospace" letterSpacing="0.06em">
          CORRELATION LAYER
        </text>
        {[
          ['ENTITIES', DEMO_CASE.entities.length],
          ['EVENTS', DEMO_CASE.events.length],
          ['LINKS', DEMO_CASE.relationships.length],
          ['FLAGS', DEMO_CASE.discrepancies.length],
        ].map(([label, value], row) => (
          <g key={label} fontFamily="JetBrains Mono, monospace" fontSize="10">
            <line x1={LAYER.x + 12} x2={layerRight - 12} y1={LAYER.y + 52 + row * 24} y2={LAYER.y + 52 + row * 24} stroke="#242428" />
            <text x={LAYER.x + 12} y={LAYER.y + 68 + row * 24} fill="#6E6E77">
              {label}
            </text>
            <text x={layerRight - 12} y={LAYER.y + 68 + row * 24} fill={label === 'FLAGS' ? '#FF5A4E' : '#EDEDEF'} textAnchor="end">
              {String(value).padStart(2, '0')}
            </text>
          </g>
        ))}
      </g>

      {/* Layer → entities */}
      {Object.entries(HERO_NODES).map(([id, node]) => (
        <path
          key={`out-${id}`}
          d={`M${layerRight} ${layerMidY} C ${layerRight + 30} ${layerMidY}, ${node.x - 40} ${node.y}, ${node.x - 6} ${node.y}`}
          fill="none"
          stroke="#EDEDEF"
          strokeOpacity={isEntityLit(id) ? 0.18 : 0.04}
          strokeWidth="1"
          style={{ transition: 'stroke-opacity 250ms' }}
        />
      ))}

      {/* Entity relationships */}
      {HERO_EDGES.map((edge) => {
        const from = HERO_NODES[edge.from];
        const to = HERO_NODES[edge.to];
        if (!from || !to) return null;
        const style = PROVENANCE_STYLES[edge.provenance];
        const lit = isEntityLit(edge.from) && isEntityLit(edge.to);
        return (
          <line
            key={`${edge.from}-${edge.to}`}
            x1={from.x}
            y1={from.y}
            x2={to.x}
            y2={to.y}
            stroke={style.stroke}
            strokeDasharray={style.dash}
            strokeOpacity={lit ? 0.9 : 0.15}
            strokeWidth="1.25"
            style={{ transition: 'stroke-opacity 250ms' }}
          />
        );
      })}

      {/* Entity nodes */}
      {Object.entries(HERO_NODES).map(([id, node], index) => {
        const lit = isEntityLit(id);
        const highlighted = activeSource?.entities.includes(id) ?? false;
        return (
          <g key={id} style={{ transition: 'opacity 250ms' }} opacity={lit ? 1 : 0.3}>
            {highlighted && (
              <rect x={node.x - 6} y={node.y - 6} width="12" height="12" fill="none" stroke="#FFB224" className="pulse-node" />
            )}
            {!activeSource && index === 0 && (
              <rect x={node.x - 6} y={node.y - 6} width="12" height="12" fill="none" stroke="#EDEDEF" className="pulse-node" />
            )}
            <rect
              x={node.x - 5}
              y={node.y - 5}
              width="10"
              height="10"
              fill={highlighted ? '#FFB224' : '#09090A'}
              stroke={highlighted ? '#FFB224' : '#EDEDEF'}
              strokeWidth="1.25"
            />
            <text
              className="svg-label"
              x={node.x}
              y={node.y + 20}
              textAnchor="middle"
              fill="#A3A3AB"
              fontSize="10"
              fontFamily="JetBrains Mono, monospace"
            >
              {node.label}
            </text>
          </g>
        );
      })}

      {/* Event sequence */}
      <g fontFamily="JetBrains Mono, monospace">
        <path
          d={`M${LAYER.x + LAYER.w / 2} ${LAYER.y + LAYER.h} V ${TIMELINE_Y - 30}`}
          stroke="#36363C"
          strokeDasharray="2 4"
        />
        <text x="214" y={TIMELINE_Y - 38} fill="#6E6E77" fontSize="9.5" letterSpacing="0.08em">
          EVENT SEQUENCE · ORDINAL
        </text>
        <line x1="200" x2="590" y1={TIMELINE_Y} y2={TIMELINE_Y} stroke="#36363C" />
        <line
          x1="200"
          x2="590"
          y1={TIMELINE_Y}
          y2={TIMELINE_Y}
          stroke="#EDEDEF"
          strokeOpacity="0.35"
          className="flow-line flow-line--slow"
        />
        {DEMO_CASE.events.map((event, index) => {
          const x = EVENT_X(index);
          const flagged = event.evidenceId === FLAGGED_EVENT;
          const lit = isEventLit(event.id);
          return (
            <g key={event.id} opacity={lit ? 1 : 0.3} style={{ transition: 'opacity 250ms' }}>
              <rect
                x={x - 4}
                y={TIMELINE_Y - 4}
                width="8"
                height="8"
                fill={flagged ? '#FF5A4E' : activeSource?.events.includes(event.id) ? '#FFB224' : '#EDEDEF'}
                transform={`rotate(45 ${x} ${TIMELINE_Y})`}
              />
              <text x={x} y={TIMELINE_Y + 22} textAnchor="middle" fill="#A3A3AB" fontSize="10">
                {event.time.slice(0, 5)}
              </text>
              <text x={x} y={TIMELINE_Y + 36} textAnchor="middle" fill="#6E6E77" fontSize="8.5">
                {event.evidenceId}
              </text>
            </g>
          );
        })}
        <text x="590" y={TIMELINE_Y - 12} textAnchor="end" fill="#FF5A4E" fontSize="9" letterSpacing="0.06em">
          ▲ CLOCK VARIANCE · REVIEW
        </text>
      </g>
    </svg>
  );
};

export interface EvidenceNetworkVisualProps {
  /** Controlled active source (optional); falls back to internal state. */
  active?: SourceId | null;
  onActiveChange?: (id: SourceId | null) => void;
}

export const EvidenceNetworkVisual: React.FC<EvidenceNetworkVisualProps> = ({ active: controlled, onActiveChange }) => {
  const [internal, setInternal] = useState<SourceId | null>(null);
  const active = controlled !== undefined ? controlled : internal;
  const setActive = (value: SourceId | null | ((current: SourceId | null) => SourceId | null)): void => {
    const next = typeof value === 'function' ? value(active) : value;
    if (controlled === undefined) setInternal(next);
    onActiveChange?.(next);
  };

  return (
    <Frame
      className="relative w-full"
      title={
        <>
          <span aria-hidden="true" className="h-1.5 w-1.5 bg-signal-amber" />
          <span className="text-fg">{DEMO_CASE.id}</span>
          <span className="hidden sm:inline">· correlation view</span>
        </>
      }
      meta="Synthetic data"
    >
      {/* Mobile: sources as chips above a cropped diagram */}
      <div className="grid grid-cols-3 gap-px border-b border-ink-line bg-ink-line sm:hidden">
        {HERO_SOURCES.map((source) => (
          <button
            key={source.id}
            type="button"
            aria-pressed={active === source.id}
            onClick={() => setActive((current) => (current === source.id ? null : source.id))}
            className={`flex flex-col items-start gap-1.5 bg-ink-900 px-3 py-2.5 text-left font-mono text-[0.625rem] uppercase leading-tight transition-colors ${
              active === source.id ? 'text-signal-amber' : 'text-fg-muted'
            }`}
          >
            <source.icon aria-hidden="true" className="h-3.5 w-3.5" />
            {source.label}
          </button>
        ))}
      </div>
      <div className="relative px-3 pb-2 pt-4 sm:px-5 sm:pt-5">
        <div aria-hidden="true" className="pattern-dots pointer-events-none absolute inset-0 opacity-40 [mask-image:radial-gradient(ellipse_at_70%_40%,black,transparent_70%)]" />
        <div className="relative sm:hidden">
          <NetworkDiagram active={active} onActivate={setActive} compact />
        </div>
        <div className="relative hidden sm:block">
          <NetworkDiagram active={active} onActivate={setActive} />
        </div>
      </div>
      <div className="flex flex-col gap-3 border-t border-ink-line px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
        <ProvenanceLegend />
        <p className="font-mono text-micro uppercase text-fg-subtle">
          <span className="hidden sm:inline">Hover a source to trace it</span>
          <span className="sm:hidden">Tap a source to trace it</span>
        </p>
      </div>
    </Frame>
  );
};
