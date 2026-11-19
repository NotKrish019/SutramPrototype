import React from 'react';
import { DEMO_CASE, PROVENANCE_LABEL, type DemoEntity, type EntityKind } from '../../../../fixtures/synthetic/case026';
import { PROVENANCE_STYLES } from '../../../../components/ui/primitives';

const MONO = 'JetBrains Mono, ui-monospace, monospace';

const KIND_LABEL: Record<EntityKind, string> = {
  person: 'Person',
  device: 'Device',
  vehicle: 'Vehicle',
  location: 'Location',
  account: 'Account',
  credential: 'Credential',
};

/** Node glyph by entity type — shape carries the type, colour carries state. */
const NodeShape: React.FC<{ kind: EntityKind; x: number; y: number; fill: string; stroke: string }> = ({
  kind,
  x,
  y,
  fill,
  stroke,
}) => {
  const common = { fill, stroke, strokeWidth: 1.5 };
  switch (kind) {
    case 'person':
      return <circle cx={x} cy={y} r={11} {...common} />;
    case 'location':
      return (
        <g>
          <rect x={x - 12} y={y - 12} width={24} height={24} {...common} />
          <rect x={x - 3} y={y - 3} width={6} height={6} fill={stroke} />
        </g>
      );
    case 'credential':
    case 'account':
      return <rect x={x - 8} y={y - 8} width={16} height={16} transform={`rotate(45 ${x} ${y})`} {...common} />;
    case 'vehicle':
      return <rect x={x - 13} y={y - 8} width={26} height={16} {...common} />;
    case 'device':
    default:
      return <rect x={x - 8} y={y - 11} width={16} height={22} rx={3} {...common} />;
  }
};

export interface KnowledgeGraphProps {
  /** Entities to emphasise (others dim). Empty or undefined = all normal. */
  highlighted?: ReadonlySet<string>;
  selectedEntityId?: string | null;
  onSelectEntity?: (entity: DemoEntity) => void;
  showEdgeLabels?: boolean;
  className?: string;
  titleId: string;
}

export const KnowledgeGraph: React.FC<KnowledgeGraphProps> = ({
  highlighted,
  selectedEntityId = null,
  onSelectEntity,
  showEdgeLabels = true,
  className = '',
  titleId,
}) => {
  const byId = new Map(DEMO_CASE.entities.map((entity) => [entity.id, entity]));
  const hasHighlight = highlighted !== undefined && highlighted.size > 0;
  const isLit = (id: string): boolean => !hasHighlight || highlighted.has(id);
  const interactive = onSelectEntity !== undefined;

  return (
    <svg viewBox="0 0 600 380" className={`h-auto w-full ${className}`} role="group" aria-labelledby={titleId}>
      <title id={titleId}>
        Knowledge graph of synthetic case {DEMO_CASE.id}: {DEMO_CASE.entities.length} entities and{' '}
        {DEMO_CASE.relationships.length} relationships.
      </title>

      {DEMO_CASE.relationships.map((rel) => {
        const from = byId.get(rel.from);
        const to = byId.get(rel.to);
        if (!from || !to) return null;
        const style = PROVENANCE_STYLES[rel.provenance];
        const lit = isLit(rel.from) && isLit(rel.to);
        const mx = (from.x + to.x) / 2;
        const my = (from.y + to.y) / 2;
        return (
          <g key={rel.id} opacity={lit ? 1 : 0.15} style={{ transition: 'opacity 250ms' }}>
            <line
              x1={from.x}
              y1={from.y}
              x2={to.x}
              y2={to.y}
              stroke={style.stroke}
              strokeOpacity={rel.provenance === 'observed' ? 0.75 : 0.95}
              strokeWidth="1.25"
              strokeDasharray={style.dash}
            >
              <title>
                {from.label} — {rel.label} → {to.label} ({PROVENANCE_LABEL[rel.provenance]})
              </title>
            </line>
            {showEdgeLabels && (
              <g>
                <rect x={mx - rel.label.length * 2.9 - 5} y={my - 8} width={rel.label.length * 5.8 + 10} height="15" fill="#11100E" />
                <text x={mx} y={my + 3} textAnchor="middle" fontFamily={MONO} fontSize="9" fill={style.stroke} fillOpacity="0.9">
                  {rel.label}
                </text>
              </g>
            )}
          </g>
        );
      })}

      {DEMO_CASE.entities.map((entity) => {
        const selected = entity.id === selectedEntityId;
        const lit = isLit(entity.id);
        const stroke = selected ? '#D8C5A7' : '#F1E9DB';
        const fill = selected ? '#2a2218' : '#080807';
        const content = (
          <>
            {selected && <circle cx={entity.x} cy={entity.y} r="22" fill="none" stroke="#D8C5A7" strokeOpacity="0.35" />}
            <NodeShape kind={entity.kind} x={entity.x} y={entity.y} fill={fill} stroke={stroke} />
            <text className="svg-label" x={entity.x} y={entity.y + 30} textAnchor="middle" fontFamily={MONO} fontSize="10.5" fill="#F1E9DB">
              {entity.label}
            </text>
            <text className="svg-label" x={entity.x} y={entity.y + 43} textAnchor="middle" fontFamily={MONO} fontSize="8.5" fill="#857D72">
              {KIND_LABEL[entity.kind].toUpperCase()}
            </text>
          </>
        );
        return (
          <g key={entity.id} opacity={lit ? 1 : 0.3} style={{ transition: 'opacity 250ms' }}>
            {interactive ? (
              <g
                role="button"
                tabIndex={0}
                aria-pressed={selected}
                aria-label={`${entity.label}, ${KIND_LABEL[entity.kind]}`}
                className="cursor-pointer focus:outline-none [&:focus-visible_circle]:stroke-signal-accent [&:focus-visible_rect]:stroke-signal-accent"
                onClick={() => onSelectEntity(entity)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    onSelectEntity(entity);
                  }
                }}
              >
                {/* Larger invisible hit target */}
                <rect x={entity.x - 40} y={entity.y - 22} width="80" height="70" fill="transparent" />
                {content}
              </g>
            ) : (
              content
            )}
          </g>
        );
      })}
    </svg>
  );
};
