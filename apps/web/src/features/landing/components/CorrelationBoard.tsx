import React, { useMemo, useState } from 'react';
import { Route } from 'lucide-react';
import { SECTION_IDS } from '../data/siteContent';
import { useInView } from '../hooks/useInView';
import { CctvStill } from '../board/artwork/CctvStill';
import { LogPrintout, NETWORK_LOG_ROWS, TransactionSlip } from '../board/artwork/Documents';
import { EditorialHeading, EvidencePrint, Pushpin, SyntheticStamp } from '../board/materials';
import { ThreadCanvas, type ThreadLink } from '../board/ThreadCanvas';

type NodeId = 'cctv' | 'comms' | 'device' | 'txn' | 'person' | 'event';

interface BoardNode {
  id: NodeId;
  name: string;
  source: string;
  x: number;
  y: number;
  w: number;
  rot: number;
}

const NODES: BoardNode[] = [
  { id: 'event', name: 'Event · Gate B entry', source: 'EVT-3 / EVT-4', x: 41, y: 12, w: 17, rot: 0 },
  { id: 'cctv', name: 'CCTV still · Gate B camera', source: 'EV-01 · observed', x: 1, y: 4, w: 24, rot: -3 },
  { id: 'comms', name: 'Communication record · message excerpt', source: 'EV-03 · observed', x: 72, y: 0, w: 17, rot: 3 },
  { id: 'device', name: 'Device record · D-01 network association', source: 'EV-05 · observed', x: 73, y: 52, w: 21, rot: -2 },
  { id: 'txn', name: 'Transaction record · card ··8812', source: 'EV-04 · supplied', x: 6, y: 60, w: 14, rot: 2.5 },
  { id: 'person', name: 'Entity · Person A', source: 'ENT-PA · supplied by HR record', x: 40, y: 60, w: 14, rot: -1.5 },
];

interface Edge {
  id: string;
  from: NodeId;
  to: NodeId;
  kind: 'observed' | 'inferred';
  label: string;
  labelT?: number;
}

/** Relationships mirror the synthetic CASE-026 graph. Supplied links are drawn solid and labelled “supplied”. */
const EDGES: Edge[] = [
  { id: 'e1', from: 'cctv', to: 'event', kind: 'observed', label: 'gate opens 22:41:07' },
  { id: 'e2', from: 'comms', to: 'device', kind: 'observed', label: 'sent from D-01' },
  { id: 'e3', from: 'device', to: 'event', kind: 'observed', label: 'at AP-WH7 22:44:10' },
  { id: 'e4', from: 'person', to: 'device', kind: 'inferred', label: 'likely user of · inferred' },
  { id: 'e5', from: 'txn', to: 'person', kind: 'observed', label: 'card held by · supplied' },
  { id: 'e6', from: 'cctv', to: 'person', kind: 'inferred', label: 'possible link · inferred', labelT: 0.62 },
];

/** The related-entity path: card → person → device → event. */
const PATH_EDGES = ['e5', 'e4', 'e3'];
const PATH_NODES: NodeId[] = ['txn', 'person', 'device', 'event'];

const nodeName = (id: NodeId): string => NODES.find((n) => n.id === id)?.name ?? id;

function renderNode(id: NodeId): React.ReactNode {
  switch (id) {
    case 'cctv':
      return (
        <EvidencePrint rot={0} pinId="c-cctv" tag="EV-01" caption="CCTV-03 · 22:41:07" imageClassName="aspect-[16/11]">
          <CctvStill annotate />
        </EvidencePrint>
      );
    case 'comms':
      return (
        <div className="relative bg-[#e6d6b8] p-3 font-mono text-[8.5px] leading-[1.6] text-[#2b2218] shadow-[inset_0_0_14px_rgba(98,66,30,.35),0_30px_40px_-16px_rgba(0,0,0,.85)] [background-image:var(--fibre),linear-gradient(170deg,#efe2c8,#d9c5a2)]">
          <Pushpin pinId="c-comms" tone="brass" className="-top-3 left-1/2 -translate-x-1/2" />
          <p className="flex justify-between border-b border-[#2b2218]/30 pb-1 text-[7px] uppercase tracking-[0.12em]">
            <span>Message excerpt</span>
            <span className="text-crimson-deep">EV-03</span>
          </p>
          <p className="mt-1.5">D-01 → ▇▇▇▇▇▇</p>
          <p>14 Mar 2026 · 22:36:14</p>
          <p className="mt-1.5 font-serif text-[13px] italic leading-tight">“at the gate in 5”</p>
        </div>
      );
    case 'device':
      return (
        <div className="relative">
          <Pushpin pinId="c-device" tone="steel" className="-top-3 left-1/2 -translate-x-1/2" />
          <LogPrintout title="Device record · D-01" file="warehouse_ap_assoc.log" tag="EV-05" rows={NETWORK_LOG_ROWS} className="shadow-[0_30px_40px_-16px_rgba(0,0,0,.85)]" />
        </div>
      );
    case 'txn':
      return (
        <div className="relative">
          <Pushpin pinId="c-txn" className="-top-3 left-1/2 -translate-x-1/2" />
          <TransactionSlip className="drop-shadow-[0_24px_24px_rgba(0,0,0,.8)]" />
        </div>
      );
    case 'person':
      return (
        <div className="relative bg-[#e6d6b8] px-3 pb-3 pt-4 text-[#2b2218] shadow-[inset_0_0_14px_rgba(98,66,30,.35),0_30px_40px_-16px_rgba(0,0,0,.85)] [background-image:var(--fibre),linear-gradient(170deg,#efe2c8,#d9c5a2)]">
          <Pushpin pinId="c-person" className="-top-3 left-1/2 -translate-x-1/2" />
          <svg viewBox="0 0 60 44" aria-hidden="true" className="mx-auto h-12 w-16">
            <circle cx="30" cy="15" r="9" fill="none" stroke="#2b2218" strokeWidth="1.2" strokeDasharray="2 2" />
            <path d="M12 44 C 12 30, 48 30, 48 44" fill="none" stroke="#2b2218" strokeWidth="1.2" strokeDasharray="2 2" />
          </svg>
          <p className="mt-1 text-center font-serif text-base leading-tight">Person A</p>
          <p className="text-center font-mono text-[7px] uppercase tracking-[0.12em] text-[#2b2218]/70">ENT-PA · badge holder</p>
          <p className="mt-2 border border-crimson-deep/60 px-1 py-0.5 text-center font-mono text-[6.5px] uppercase tracking-[0.1em] text-crimson-deep">
            Identity not established
          </p>
        </div>
      );
    case 'event':
      return (
        <div className="flex flex-col items-center text-center">
          <span
            data-pin-id="c-event"
            className="relative grid h-20 w-20 place-items-center rounded-full bg-crimson font-mono text-[0.625rem] uppercase tracking-[0.14em] text-white shadow-[0_0_0_6px_rgba(227,38,54,.15),0_20px_40px_-10px_rgba(227,38,54,.6)]"
          >
            <span className="absolute inset-1.5 rounded-full border border-white/40" />
            Event
          </span>
          <span className="mt-3 font-serif text-lg text-fg-bright">Gate B entry</span>
          <span className="font-mono text-[0.625rem] uppercase tracking-[0.12em] text-fg-subtle">22:41 – 22:44 · WH-7</span>
        </div>
      );
  }
}

export const CorrelationBoard: React.FC = () => {
  const [ref, visible] = useInView<HTMLDivElement>({ rootMargin: '0px 0px -20% 0px' });
  const [focus, setFocus] = useState<NodeId | null>(null);
  const [tracing, setTracing] = useState(false);

  const { links, activeNodes } = useMemo(() => {
    const hotEdges = new Set<string>();
    const nodes = new Set<NodeId>();
    if (tracing) {
      PATH_EDGES.forEach((e) => hotEdges.add(e));
      PATH_NODES.forEach((n) => nodes.add(n));
    } else if (focus) {
      nodes.add(focus);
      for (const edge of EDGES) {
        if (edge.from === focus || edge.to === focus) {
          hotEdges.add(edge.id);
          nodes.add(edge.from);
          nodes.add(edge.to);
        }
      }
    }
    const anyActive = hotEdges.size > 0;
    const result: ThreadLink[] = EDGES.map((edge, i) => ({
      id: edge.id,
      from: `c-${edge.from}`,
      to: `c-${edge.to}`,
      kind: edge.kind,
      label: edge.label,
      labelT: edge.labelT,
      delay: tracing ? PATH_EDGES.indexOf(edge.id) * 200 : i * 180,
      state: !anyActive ? 'idle' : hotEdges.has(edge.id) ? 'hot' : 'dim',
    }));
    return { links: result, activeNodes: nodes };
  }, [focus, tracing]);

  const connections = focus ? EDGES.filter((e) => e.from === focus || e.to === focus) : [];

  const readout = tracing
    ? 'Related-entity path: card ··8812 → Person A → device D-01 → Gate B entry. One link is inferred and requires review.'
    : focus
      ? `${nodeName(focus)} — ${connections.length} connection${connections.length === 1 ? '' : 's'}: ${connections
          .map((e) => `${e.label} (${nodeName(e.from === focus ? e.to : e.from)})`)
          .join('; ')}.`
      : 'Hover or select a record to see what it connects to.';

  return (
    <section id={SECTION_IDS.correlation} aria-labelledby="correlation-heading" className="relative overflow-hidden border-b border-ink-line bg-ink-950 py-section">
      <div aria-hidden="true" className="pattern-dots pointer-events-none absolute inset-0 opacity-[0.18] [mask-image:radial-gradient(ellipse_at_center,black,transparent_75%)]" />
      <div className="relative mx-auto max-w-wide px-gutter">
        <div className="grid gap-10 lg:grid-cols-12 lg:items-end">
          <EditorialHeading
            id="correlation-heading"
            index="02"
            kicker="Evidence correlation"
            title="Different sources."
            emphasis="Connected evidence."
            lead="Reveal relationships across records, entities and events."
            className="lg:col-span-7"
          />
          <div className="lg:col-span-5">
            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                aria-pressed={tracing}
                onClick={() => setTracing((t) => !t)}
                className={`inline-flex items-center gap-2 border px-4 py-2.5 font-mono text-[0.6875rem] uppercase tracking-[0.12em] transition-colors duration-base focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson ${
                  tracing ? 'border-crimson bg-crimson text-white' : 'border-paper/30 text-fg hover:border-paper/70'
                }`}
              >
                <Route aria-hidden="true" className="h-4 w-4" />
                {tracing ? 'Clear path' : 'Trace related path'}
              </button>
              <SyntheticStamp />
            </div>
            <p aria-live="polite" className="mt-4 min-h-[3rem] border-l border-crimson/60 pl-4 text-sm leading-relaxed text-fg-muted">
              {readout}
            </p>
            <ul aria-label="Thread legend" className="mt-3 flex gap-5 font-mono text-[0.625rem] uppercase tracking-[0.12em] text-fg-subtle">
              <li className="flex items-center gap-2">
                <svg aria-hidden="true" width="26" height="4"><line x1="0" y1="2" x2="26" y2="2" stroke="#c81f2e" strokeWidth="1.6" /></svg>
                Observed / supplied
              </li>
              <li className="flex items-center gap-2">
                <svg aria-hidden="true" width="26" height="4"><line x1="0" y1="2" x2="26" y2="2" stroke="#c81f2e" strokeWidth="1.6" strokeDasharray="6 5" /></svg>
                Inferred — review
              </li>
            </ul>
          </div>
        </div>

        <div ref={ref} className="scene-3d relative mt-16 lg:h-[46rem]">
          <ThreadCanvas links={links} drawn={visible} sagAmount={0.08} labelClassName="hidden lg:block" />
          <ul className="relative grid grid-cols-2 gap-x-6 gap-y-14 sm:gap-x-10 lg:block lg:h-full">
            {NODES.map((node) => {
              const active = activeNodes.has(node.id);
              const dimmed = activeNodes.size > 0 && !active;
              return (
                <li
                  key={node.id}
                  className={`relative lg:absolute lg:left-[var(--x)] lg:top-[var(--y)] lg:w-[var(--w)] ${node.id === 'event' ? 'col-span-2 mx-auto' : ''} ${
                    node.id === 'cctv' ? 'col-span-2 mx-auto w-4/5 sm:w-3/5 lg:w-auto' : ''
                  }`}
                  style={{ '--x': `${node.x}%`, '--y': `${node.y}%`, '--w': `${node.w}%` } as React.CSSProperties}
                >
                  <button
                    type="button"
                    aria-pressed={focus === node.id}
                    aria-label={`${node.name} (${node.source})`}
                    onMouseEnter={() => {
                      setTracing(false);
                      setFocus(node.id);
                    }}
                    onMouseLeave={() => setFocus(null)}
                    onFocus={() => {
                      setTracing(false);
                      setFocus(node.id);
                    }}
                    onBlur={() => setFocus(null)}
                    onClick={() => setFocus((f) => (f === node.id ? null : node.id))}
                    data-active={active}
                    className={`is-liftable preserve-3d block w-full text-left transition-[opacity,filter] duration-slow focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson focus-visible:ring-offset-4 focus-visible:ring-offset-ink-950 ${
                      dimmed ? 'opacity-40 saturate-50' : 'opacity-100'
                    }`}
                  >
                    <span
                      className={`block transition-transform duration-slow ease-out ${active ? '-translate-y-2' : ''}`}
                      style={{ transform: `rotate(${node.rot}deg)` }}
                    >
                      {renderNode(node.id)}
                    </span>
                  </button>
                  {node.id !== 'event' && (
                    <p className={`mt-4 font-mono text-[0.625rem] uppercase tracking-[0.12em] transition-colors ${active ? 'text-paper' : 'text-fg-subtle'}`}>
                      {node.source}
                    </p>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </section>
  );
};
