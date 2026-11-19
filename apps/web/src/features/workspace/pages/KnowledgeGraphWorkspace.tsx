import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowRight, ChevronDown, Focus, Maximize2, Minus, Plus, RotateCcw, Waypoints } from 'lucide-react';
import { ProvenanceBadge, ReviewBadge } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { useCaseIndex, useQuerySelection, type CaseIndex } from '../caseIndex';
import { FilterChip, KeyValueList, Panel, SearchField, SelectField, WsButton } from '../components/ui';
import { formatEventTime } from '../format';
import {
  CATEGORY_LABEL,
  EDGE_KIND_LABEL,
  buildGraph,
  shortestPath,
  type EdgeKind,
  type GraphEdge,
  type GraphNode,
  type NodeCategory,
} from '../graph/graphModel';
import type { CaseDetail } from '../types';

const MONO = 'JetBrains Mono, ui-monospace, monospace';
const CANVAS = { w: 1000, h: 640 };
const DEFAULT_VIEW = { x: 0, y: 0, w: CANVAS.w, h: CANVAS.h };
type View = typeof DEFAULT_VIEW;

const EDGE_STYLE: Record<GraphEdge['provenance'], { stroke: string; dash?: string; width: number; opacity: number }> = {
  observed: { stroke: '#F1E9DB', width: 1.4, opacity: 0.75 },
  supplied: { stroke: '#8FA9C4', dash: '2 5', width: 1.6, opacity: 0.95 },
  inferred: { stroke: '#D8C5A7', dash: '7 6', width: 1.5, opacity: 0.95 },
  structural: { stroke: '#857D72', width: 1, opacity: 0.45 },
};

/** Restrained per-type tones from the YUKTI palette; crimson is reserved for contradictions. */
export const NODE_TONE: Record<NodeCategory, string> = {
  person: '#F1E9DB',
  device: '#8FA9C4',
  vehicle: '#D8C5A7',
  location: '#C9A45A',
  organization: '#B9A584',
  unclassified: '#857D72',
  event: '#A9A197',
  evidence: '#6F8AA6',
};

const NodeGlyph: React.FC<{ node: GraphNode; fill: string; stroke: string }> = ({ node, fill, stroke }) => {
  const { x, y } = node;
  const common = { fill, stroke, strokeWidth: 1.6 };
  switch (node.category) {
    case 'person':
      return <circle cx={x} cy={y} r={14} {...common} />;
    case 'device':
      return <rect x={x - 10} y={y - 14} width={20} height={28} rx={4} {...common} />;
    case 'vehicle':
      return <rect x={x - 17} y={y - 10} width={34} height={20} {...common} />;
    case 'location':
      return (
        <g>
          <rect x={x - 14} y={y - 14} width={28} height={28} {...common} />
          <rect x={x - 4} y={y - 4} width={8} height={8} fill={stroke} />
        </g>
      );
    case 'organization':
      return <polygon points={`${x - 14},${y} ${x - 7},${y - 13} ${x + 7},${y - 13} ${x + 14},${y} ${x + 7},${y + 13} ${x - 7},${y + 13}`} {...common} />;
    case 'event':
      return <rect x={x - 9} y={y - 9} width={18} height={18} transform={`rotate(45 ${x} ${y})`} {...common} />;
    case 'evidence':
      return <path d={`M${x - 10} ${y - 13} h14 l6 6 v20 h-20 z`} {...common} />;
    case 'unclassified':
    default:
      return <rect x={x - 10} y={y - 10} width={20} height={20} transform={`rotate(45 ${x} ${y})`} {...common} strokeDasharray="3 2" />;
  }
};

/* ----------------------------------------------------------------- Inspector */

const InspectorSection: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <div>
    <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle">{title}</h3>
    {children}
  </div>
);

const Inspector: React.FC<{
  detail: CaseDetail;
  index: CaseIndex;
  nodes: GraphNode[];
  edges: GraphEdge[];
  node: GraphNode | null;
  edge: GraphEdge | null;
  pathTarget: string;
  onPathTarget: (id: string) => void;
  pathFound: boolean | null;
  onSelectNode: (id: string) => void;
  onSelectEdge: (id: string) => void;
}> = ({ detail, index, nodes, edges, node, edge, pathTarget, onPathTarget, pathFound, onSelectNode, onSelectEdge }) => {
  const caseId = detail.case.case_id;
  if (!node && !edge) {
    return (
      <div className="px-4 py-8 text-center">
        <Waypoints aria-hidden="true" strokeWidth={1.2} className="mx-auto h-8 w-8 text-fg-subtle" />
        <p className="mt-3 text-sm text-fg-muted">Select a node or link.</p>
      </div>
    );
  }

  if (edge) {
    const rel = detail.relationships.find((r) => r.relationship_id === edge.id);
    const review = detail.review_items.find((item) => rel && item.related_event_ids.includes(rel.source_id) && item.related_event_ids.includes(rel.target_id));
    return (
      <div>
        <div className="border-b border-ink-line px-4 py-4">
          <p className="font-mono text-micro uppercase text-fg-subtle">Relationship · {EDGE_KIND_LABEL[edge.kind]}</p>
          <p className="mt-1 text-base text-fg">
            {index.nameOf(edge.source)} <span className="text-fg-muted">— {edge.label} →</span> {index.nameOf(edge.target)}
          </p>
          <div className="mt-2">{edge.provenance === 'structural' ? <span className="font-mono text-micro uppercase text-fg-subtle">Structural link from record fields</span> : <ProvenanceBadge value={edge.provenance} withDescription />}</div>
        </div>
        <KeyValueList
          items={[
            ['relation_type', <code key="t" className="font-mono text-xs">{edge.kind}</code>],
            ...(rel
              ? ([
                  ['is_observed', String(rel.is_observed)],
                  ['weight', rel.weight === null ? <span key="w" className="text-fg-subtle">Not provided</span> : rel.weight.toFixed(2)],
                ] as [string, React.ReactNode][])
              : []),
          ]}
        />
        {rel && (
          <InspectorSection title="Evidence basis">
            {rel.evidence_basis.length === 0 ? (
              <p className="px-4 py-3 text-sm text-fg-subtle">Supplied by a record outside the case evidence set (e.g. HR record) — needs corroboration.</p>
            ) : (
              <ul className="divide-y divide-ink-line">
                {rel.evidence_basis.map((id) => (
                  <li key={id}>
                    <Link to={caseHref(caseId, 'evidence', { evidence: id })} className="flex items-center justify-between px-4 py-2.5 text-sm text-fg hover:bg-white/[0.02]">
                      <span>{id} · <span className="text-fg-muted">{index.evidence.get(id)?.original_filename}</span></span>
                      <ArrowRight aria-hidden="true" className="h-3 w-3 text-fg-subtle" />
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </InspectorSection>
        )}
        {review && (
          <InspectorSection title="Review">
            <Link to={caseHref(caseId, 'contradictions', { item: review.id })} className="flex items-center justify-between gap-2 px-4 py-3 hover:bg-white/[0.02]">
              <span className="text-sm text-fg">{review.id} · {review.title}</span>
              <ReviewBadge value={review.status} />
            </Link>
          </InspectorSection>
        )}
      </div>
    );
  }

  const n = node!;
  const connected = edges.filter((e) => e.source === n.id || e.target === n.id);
  const entity = index.entities.get(n.id);
  const event = index.events.get(n.id);
  const evidence = index.evidence.get(n.id);
  const reviews = detail.review_items.filter((item) => item.related_entity_ids.includes(n.id) || item.related_event_ids.includes(n.id));

  return (
    <div>
      <div className="border-b border-ink-line px-4 py-4">
        <p className="font-mono text-micro uppercase text-fg-subtle">{CATEGORY_LABEL[n.category]} · {n.id}</p>
        <p className="mt-1 text-lg text-fg">{n.label}</p>
      </div>
      {entity && (
        <KeyValueList
          items={[
            ['entity_type', entity.entity_type ?? <span key="u" className="text-signal-accent">{entity.type_label}</span>],
            ...(entity.aliases.length > 0 ? ([['Aliases', entity.aliases.join(', ')]] as [string, string][]) : []),
            ...Object.entries(entity.attributes).map(([k, v]) => [k, v] as [string, string]),
            ['Confidence', entity.confidence === null ? <span key="c" className="text-fg-subtle">Not provided</span> : entity.confidence.toFixed(2)],
          ]}
        />
      )}
      {event && (
        <KeyValueList
          items={[
            ['Time', `${formatEventTime(event.timestamp, event.timestamp_precision)} IST`],
            ['Precision', event.timestamp_precision],
            ['Ordering', event.ordering_status],
            ['Recorded as', event.source_recorded_time],
          ]}
        />
      )}
      {evidence && (
        <KeyValueList
          items={[
            ['File', evidence.original_filename],
            ['Type', evidence.source_type ?? evidence.source_type_label],
            ['Source', evidence.source],
          ]}
        />
      )}

      <InspectorSection title={`Connected records · ${connected.length}`}>
        <ul className="divide-y divide-ink-line">
          {connected.map((e) => {
            const otherId = e.source === n.id ? e.target : e.source;
            return (
              <li key={e.id} className="flex items-center justify-between gap-2 px-4 py-2">
                <button type="button" onClick={() => onSelectNode(otherId)} className="min-w-0 text-left text-sm text-fg hover:text-signal-accent">
                  <span className="block truncate">{index.nameOf(otherId)}</span>
                  <span className="font-mono text-micro uppercase text-fg-subtle">{e.label}</span>
                </button>
                <button type="button" onClick={() => onSelectEdge(e.id)} className="shrink-0" aria-label={`Inspect relationship ${e.label}`}>
                  {e.provenance === 'structural' ? <span className="font-mono text-micro uppercase text-fg-subtle">link</span> : <ProvenanceBadge value={e.provenance} />}
                </button>
              </li>
            );
          })}
        </ul>
      </InspectorSection>

      <InspectorSection title="Supporting evidence">
        {(() => {
          const ids = entity ? entity.source_evidence_ids : event ? [event.source_evidence_id] : evidence ? [evidence.evidence_id] : [];
          return ids.length === 0 ? (
            <p className="px-4 py-3 text-sm text-fg-subtle">No supporting artifact in this case.</p>
          ) : (
            <ul className="divide-y divide-ink-line">
              {ids.map((id) => (
                <li key={id}>
                  <Link to={caseHref(caseId, 'evidence', { evidence: id })} className="flex items-center justify-between px-4 py-2.5 text-sm text-fg hover:bg-white/[0.02]">
                    <span className="truncate">{id} · <span className="text-fg-muted">{index.evidence.get(id)?.original_filename}</span></span>
                    <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />
                  </Link>
                </li>
              ))}
            </ul>
          );
        })()}
      </InspectorSection>

      {reviews.length > 0 && (
        <InspectorSection title="Review status">
          <ul className="divide-y divide-ink-line">
            {reviews.map((item) => (
              <li key={item.id}>
                <Link to={caseHref(caseId, 'contradictions', { item: item.id })} className="flex items-center justify-between gap-2 px-4 py-2.5 hover:bg-white/[0.02]">
                  <span className="truncate text-sm text-fg">{item.title}</span>
                  <ReviewBadge value={item.status} />
                </Link>
              </li>
            ))}
          </ul>
        </InspectorSection>
      )}

      <InspectorSection title="Trace path">
        <div className="space-y-2 px-4 py-3">
          <SelectField
            label="To"
            value={pathTarget}
            onChange={onPathTarget}
            options={[{ value: '', label: 'Choose a node…' }, ...nodes.filter((other) => other.id !== n.id).map((other) => ({ value: other.id, label: other.label }))]}
          />
          {pathFound === false && <p className="text-xs text-signal-flag">No path through the visible relationships.</p>}
          {pathFound === true && <p className="text-xs text-fg-muted">Path highlighted on the canvas.</p>}
        </div>
      </InspectorSection>
      {event && (
        <div className="border-t border-ink-line px-4 py-3">
          <Link to={caseHref(caseId, 'timeline', { event: event.event_id })} className="font-mono text-micro uppercase text-signal-accent hover:underline">
            Open in timeline →
          </Link>
        </div>
      )}
    </div>
  );
};

/* --------------------------------------------------------------------- Page */

export const KnowledgeGraphWorkspace: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const index = useCaseIndex(detail);
  const graph = useMemo(() => buildGraph(detail), [detail]);
  const [nodeId, setNodeId] = useQuerySelection('node');
  const [edgeId, setEdgeId] = useQuerySelection('edge');
  const [view, setView] = useState<View>(DEFAULT_VIEW);
  const [hiddenCategories, setHiddenCategories] = useState<Set<NodeCategory>>(() => new Set(['evidence']));
  const [hiddenKinds, setHiddenKinds] = useState<Set<EdgeKind>>(() => new Set());
  const [search, setSearch] = useState('');
  const [pathTarget, setPathTarget] = useState('');
  const svgRef = useRef<SVGSVGElement>(null);
  const drag = useRef<{ x: number; y: number; view: View } | null>(null);
  const viewRef = useRef(view);
  viewRef.current = view;

  const categories = useMemo(() => [...new Set(graph.nodes.map((n) => n.category))], [graph.nodes]);
  const kinds = useMemo(() => [...new Set(graph.edges.map((e) => e.kind))], [graph.edges]);

  const visibleNodes = graph.nodes.filter((n) => !hiddenCategories.has(n.category));
  const visibleIds = new Set(visibleNodes.map((n) => n.id));
  const visibleEdges = graph.edges.filter((e) => !hiddenKinds.has(e.kind) && visibleIds.has(e.source) && visibleIds.has(e.target));
  const byId = new Map(graph.nodes.map((n) => [n.id, n]));

  const selectedNode = nodeId ? byId.get(nodeId) ?? null : null;
  const selectedEdge = edgeId ? graph.edges.find((e) => e.id === edgeId) ?? null : null;

  const path = selectedNode && pathTarget ? shortestPath(visibleEdges, selectedNode.id, pathTarget) : null;
  const pathEdges = new Set(path ?? []);
  const pathNodes = new Set<string>();
  for (const id of pathEdges) {
    const e = graph.edges.find((edge) => edge.id === id);
    if (e) pathNodes.add(e.source).add(e.target);
  }

  const neighbours = new Set<string>();
  if (selectedNode) {
    neighbours.add(selectedNode.id);
    for (const e of visibleEdges) {
      if (e.source === selectedNode.id) neighbours.add(e.target);
      if (e.target === selectedNode.id) neighbours.add(e.source);
    }
  }
  if (selectedEdge) neighbours.add(selectedEdge.source).add(selectedEdge.target);

  const query = search.trim().toLowerCase();
  const matches = query ? visibleNodes.filter((n) => `${n.label} ${n.sublabel} ${n.id}`.toLowerCase().includes(query)) : [];
  const matchIds = new Set(matches.map((n) => n.id));

  const nodeLit = (id: string): boolean => {
    if (pathNodes.size > 0) return pathNodes.has(id);
    if (query) return matchIds.has(id);
    if (neighbours.size > 0) return neighbours.has(id);
    return true;
  };

  const selectNode = useCallback(
    (id: string | null) => {
      setEdgeId(null);
      setNodeId(id);
      setPathTarget('');
    },
    [setEdgeId, setNodeId],
  );
  const selectEdge = useCallback(
    (id: string | null) => {
      setNodeId(null);
      setEdgeId(id);
      setPathTarget('');
    },
    [setEdgeId, setNodeId],
  );

  const zoom = (factor: number, cx = view.x + view.w / 2, cy = view.y + view.h / 2): void => {
    setView((v) => {
      const w = Math.min(Math.max(v.w * factor, 250), CANVAS.w * 2);
      const h = (w / CANVAS.w) * CANVAS.h;
      return { x: cx - ((cx - v.x) * w) / v.w, y: cy - ((cy - v.y) * h) / v.h, w, h };
    });
  };

  const fit = (): void => {
    if (visibleNodes.length === 0) return setView(DEFAULT_VIEW);
    const xs = visibleNodes.map((n) => n.x);
    const ys = visibleNodes.map((n) => n.y);
    const pad = 70;
    const minX = Math.min(...xs) - pad;
    const minY = Math.min(...ys) - pad;
    let w = Math.max(...xs) - minX + pad;
    let h = Math.max(...ys) - minY + pad;
    // keep aspect ratio
    if (w / h > CANVAS.w / CANVAS.h) h = (w * CANVAS.h) / CANVAS.w;
    else w = (h * CANVAS.w) / CANVAS.h;
    setView({ x: minX, y: minY, w, h });
  };

  const focusOn = (node: GraphNode): void => {
    selectNode(node.id);
    setView((v) => ({ ...v, x: node.x - v.w / 2, y: node.y - v.h / 2 }));
  };

  // Ctrl/⌘ + wheel (and trackpad pinch) zooms the canvas. A native non-passive listener is
  // required so preventDefault stops the browser from zooming the whole page.
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const onWheel = (event: WheelEvent): void => {
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      const rect = svg.getBoundingClientRect();
      const v = viewRef.current;
      const cx = rect.width ? v.x + ((event.clientX - rect.left) / rect.width) * v.w : v.x + v.w / 2;
      const cy = rect.height ? v.y + ((event.clientY - rect.top) / rect.height) * v.h : v.y + v.h / 2;
      const factor = event.deltaY > 0 ? 1.12 : 0.89;
      setView((current) => {
        const w = Math.min(Math.max(current.w * factor, 250), CANVAS.w * 2);
        const h = (w / CANVAS.w) * CANVAS.h;
        return { x: cx - ((cx - current.x) * w) / current.w, y: cy - ((cy - current.y) * h) / current.h, w, h };
      });
    };
    svg.addEventListener('wheel', onWheel, { passive: false });
    return () => svg.removeEventListener('wheel', onWheel);
  }, []);

  const toggle = <T,>(set: Set<T>, value: T): Set<T> => {
    const next = new Set(set);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    return next;
  };

  const scale = view.w / CANVAS.w;

  return (
    <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
      <div className="min-w-0 space-y-4 xl:col-span-9">
        <Panel
          title="Knowledge graph"
          meta={`${visibleNodes.length} nodes · ${visibleEdges.length} links`}
          actions={
            <>
              <WsButton size="sm" variant="ghost" icon={Plus} aria-label="Zoom in" onClick={() => zoom(0.8)} />
              <WsButton size="sm" variant="ghost" icon={Minus} aria-label="Zoom out" onClick={() => zoom(1.25)} />
              <WsButton size="sm" variant="ghost" icon={Maximize2} aria-label="Fit graph" onClick={fit} />
              <WsButton size="sm" variant="ghost" icon={RotateCcw} aria-label="Reset view" onClick={() => setView(DEFAULT_VIEW)} />
            </>
          }
        >
          <div className="flex flex-col gap-2 border-b border-ink-line p-3 md:flex-row md:items-center">
            <SearchField value={search} onChange={setSearch} label="Search nodes" placeholder="Search nodes" className="md:w-64" />
            {query && (
              <div className="flex items-center gap-2">
                <span className="font-mono text-micro uppercase text-fg-subtle" aria-live="polite">{matches.length} match{matches.length === 1 ? '' : 'es'}</span>
                {matches[0] && (
                  <WsButton size="sm" icon={Focus} onClick={() => focusOn(matches[0]!)}>
                    Focus {matches[0].label}
                  </WsButton>
                )}
              </div>
            )}
            <span className="hidden font-mono text-micro uppercase text-fg-subtle md:ml-auto md:inline">Drag · Ctrl + scroll</span>
          </div>

          <div className="relative overflow-x-auto bg-ink-950">
            <div aria-hidden="true" className="pattern-grid pointer-events-none absolute inset-0 min-w-[640px] opacity-50" />
            <svg
              ref={svgRef}
              viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`}
              className="relative block aspect-[1000/640] w-full min-w-[640px] cursor-grab touch-pan-y select-none active:cursor-grabbing"
              role="group"
              aria-label={`Knowledge graph for ${detail.case.case_id}. Use the node buttons to select; details appear in the inspector.`}
              onPointerDown={(event) => {
                if ((event.target as Element).closest('[data-graph-item]')) return;
                drag.current = { x: event.clientX, y: event.clientY, view };
                (event.currentTarget as Element).setPointerCapture?.(event.pointerId);
              }}
              onPointerMove={(event) => {
                if (!drag.current || !svgRef.current) return;
                const rect = svgRef.current.getBoundingClientRect();
                const dx = ((event.clientX - drag.current.x) / rect.width) * drag.current.view.w;
                const dy = ((event.clientY - drag.current.y) / rect.height) * drag.current.view.h;
                setView({ ...drag.current.view, x: drag.current.view.x - dx, y: drag.current.view.y - dy });
              }}
              onPointerUp={(event) => {
                const moved = drag.current && Math.hypot(event.clientX - drag.current.x, event.clientY - drag.current.y) > 4;
                drag.current = null;
                if (!moved && !(event.target as Element).closest('[data-graph-item]')) selectNode(null);
              }}
            >
              {visibleEdges.map((edge) => {
                const a = byId.get(edge.source)!;
                const b = byId.get(edge.target)!;
                const style = EDGE_STYLE[edge.provenance];
                const onPath = pathEdges.has(edge.id);
                const selected = selectedEdge?.id === edge.id;
                const lit = nodeLit(edge.source) && nodeLit(edge.target);
                const contradiction = edge.kind === 'CONTRADICTS';
                const mx = (a.x + b.x) / 2;
                const my = (a.y + b.y) / 2;
                return (
                  <g key={edge.id} opacity={lit || selected ? 1 : 0.12} style={{ transition: 'opacity 200ms' }}>
                    {contradiction ? (
                      <>
                        <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke="#E32636" strokeWidth={5 * Math.max(scale, 0.6)} strokeOpacity="0.18" />
                        <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke="#E32636" strokeWidth="1.6" strokeDasharray="10 4 2 4" />
                      </>
                    ) : (
                      <line
                        x1={a.x}
                        y1={a.y}
                        x2={b.x}
                        y2={b.y}
                        stroke={onPath ? '#D8C5A7' : style.stroke}
                        strokeOpacity={onPath || selected ? 1 : style.opacity}
                        strokeWidth={onPath || selected ? style.width + 1.2 : style.width}
                        strokeDasharray={style.dash}
                      />
                    )}
                    <line
                      data-graph-item
                      x1={a.x}
                      y1={a.y}
                      x2={b.x}
                      y2={b.y}
                      stroke="transparent"
                      strokeWidth="14"
                      className="cursor-pointer"
                      onClick={() => selectEdge(edge.id)}
                    >
                      <title>{`${a.label} — ${edge.label} → ${b.label}`}</title>
                    </line>
                    {(contradiction || selected || (edge.provenance !== 'structural' && scale < 0.85)) && (
                      <text x={mx} y={my - 6} textAnchor="middle" className="svg-label pointer-events-none" fontFamily={MONO} fontSize={10 * Math.max(scale, 0.7)} fill={contradiction ? '#E32636' : style.stroke}>
                        {contradiction ? '≠ ' : ''}
                        {edge.label}
                      </text>
                    )}
                  </g>
                );
              })}

              {visibleNodes.map((node) => {
                const selected = selectedNode?.id === node.id;
                const lit = nodeLit(node.id);
                const stroke = NODE_TONE[node.category];
                const fill = selected ? '#2a2218' : '#11100E';
                return (
                  <g
                    key={node.id}
                    data-graph-item
                    role="button"
                    tabIndex={0}
                    aria-pressed={selected}
                    aria-label={`${node.label}, ${CATEGORY_LABEL[node.category]}`}
                    opacity={lit ? 1 : 0.22}
                    style={{ transition: 'opacity 200ms' }}
                    className="cursor-pointer focus:outline-none [&:focus-visible_.focus-ring]:opacity-100"
                    onClick={() => selectNode(selected ? null : node.id)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        selectNode(selected ? null : node.id);
                      }
                    }}
                  >
                    <circle className="focus-ring" cx={node.x} cy={node.y} r="26" fill="none" stroke="#D8C5A7" strokeDasharray="3 3" opacity="0" />
                    {selected && (
                      <>
                        <circle cx={node.x} cy={node.y} r="30" fill={stroke} fillOpacity="0.08" />
                        <circle cx={node.x} cy={node.y} r="24" fill="none" stroke={stroke} strokeOpacity="0.6" />
                      </>
                    )}
                    {matchIds.has(node.id) && <circle cx={node.x} cy={node.y} r="22" fill="none" stroke="#D8C5A7" strokeWidth="1.5" />}
                    <rect x={node.x - 45} y={node.y - 22} width="90" height="64" fill="transparent" />
                    <NodeGlyph node={node} fill={fill} stroke={stroke} />
                    <text x={node.x} y={node.y + 30} textAnchor="middle" className="svg-label" fontFamily={MONO} fontSize="11" fill="#F1E9DB">
                      {node.label.length > 26 ? `${node.label.slice(0, 25)}…` : node.label}
                    </text>
                    <text x={node.x} y={node.y + 43} textAnchor="middle" className="svg-label" fontFamily={MONO} fontSize="8.5" fill="#857D72">
                      {node.sublabel.toUpperCase()}
                    </text>
                  </g>
                );
              })}
            </svg>
            <ul
              aria-label="Line legend"
              className="relative flex flex-wrap gap-x-4 gap-y-1 border-t border-ink-line px-3 py-2 font-mono text-[0.625rem] uppercase tracking-[0.08em] text-fg-muted"
            >
              {(['observed', 'supplied', 'inferred'] as const).map((key) => (
                <li key={key} className="flex items-center gap-1.5">
                  <svg aria-hidden="true" width="20" height="6"><line x1="0" y1="3" x2="20" y2="3" stroke={EDGE_STYLE[key].stroke} strokeWidth="1.6" strokeDasharray={EDGE_STYLE[key].dash} /></svg>
                  {key}
                </li>
              ))}
              <li className="flex items-center gap-1.5 text-crimson">
                <svg aria-hidden="true" width="20" height="6"><line x1="0" y1="3" x2="20" y2="3" stroke="#E32636" strokeWidth="1.6" strokeDasharray="10 4 2 4" /></svg>
                Contradicts (needs review)
              </li>
              <li className="normal-case tracking-normal text-fg-subtle">Weights shown only when provided</li>
            </ul>
          </div>

          <div className="flex flex-col gap-3 border-t border-ink-line p-3 lg:flex-row lg:items-start lg:justify-between">
            <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filter node types">
              {categories.map((category) => (
                <FilterChip key={category} active={!hiddenCategories.has(category)} onClick={() => setHiddenCategories((s) => toggle(s, category))}>
                  <span className="inline-flex items-center gap-1.5">
                    <span aria-hidden="true" className="h-2 w-2" style={{ background: NODE_TONE[category], opacity: hiddenCategories.has(category) ? 0.35 : 1 }} />
                    {CATEGORY_LABEL[category]}
                  </span>
                </FilterChip>
              ))}
            </div>
            <details className="group shrink-0 lg:text-right">
              <summary className="inline-flex cursor-pointer list-none items-center gap-1.5 font-mono text-micro uppercase text-fg-subtle hover:text-fg focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent">
                Relationship types · {kinds.length - hiddenKinds.size}/{kinds.length}
                <ChevronDown aria-hidden="true" className="h-3 w-3 transition-transform group-open:rotate-180" />
              </summary>
              <div className="mt-2 flex flex-wrap gap-1.5 lg:max-w-md lg:justify-end" role="group" aria-label="Filter relationship types">
                {kinds.map((kind) => (
                  <FilterChip key={kind} active={!hiddenKinds.has(kind)} onClick={() => setHiddenKinds((s) => toggle(s, kind))}>
                    {EDGE_KIND_LABEL[kind]}
                  </FilterChip>
                ))}
              </div>
            </details>
          </div>
        </Panel>
      </div>

      <Panel as="aside" title="Inspector" className="ws-inspector-enter xl:col-span-3 xl:self-start xl:sticky xl:top-4">
        <Inspector
          detail={detail}
          index={index}
          nodes={visibleNodes}
          edges={visibleEdges}
          node={selectedNode}
          edge={selectedEdge}
          pathTarget={pathTarget}
          onPathTarget={setPathTarget}
          pathFound={pathTarget ? path !== null : null}
          onSelectNode={(id) => selectNode(id)}
          onSelectEdge={(id) => selectEdge(id)}
        />
      </Panel>
    </div>
  );
};
