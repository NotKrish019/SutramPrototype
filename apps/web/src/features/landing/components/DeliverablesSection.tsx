import React from 'react';
import {
  DELIVERABLES,
  DELIVERABLE_STATUS_LABEL,
  SECTION_IDS,
  type Deliverable,
  type DeliverableStatus,
} from '../data/siteContent';
import { Container, Reveal, SectionHeader } from '../../../components/ui/primitives';

const STATUS_STYLE: Record<DeliverableStatus, string> = {
  implemented: 'border-signal-ok/50 text-signal-ok',
  'in-development': 'border-signal-amber/50 text-signal-amber',
  prototype: 'border-signal-supplied/50 text-signal-supplied',
  planned: 'border-ink-strong text-fg-muted',
};

const MONO = 'JetBrains Mono, ui-monospace, monospace';

const CorrelationDiagram: React.FC = () => (
  <svg viewBox="0 0 520 140" className="h-full w-full" aria-hidden="true">
    {[20, 60, 100].map((y, i) => (
      <g key={y}>
        <rect x="8" y={y - 12} width="120" height="24" fill="#0F0F11" stroke="#36363C" />
        <text x="20" y={y + 4} fontFamily={MONO} fontSize="10" fill="#A3A3AB">{['device log', 'access log', 'cctv index'][i]}</text>
        <path d={`M128 ${y} C 190 ${y}, 200 70, 250 70`} fill="none" stroke="#36363C" />
        <path d={`M128 ${y} C 190 ${y}, 200 70, 250 70`} fill="none" stroke="#EDEDEF" strokeOpacity="0.5" className="flow-line flow-line--slow" />
      </g>
    ))}
    <rect x="250" y="44" width="80" height="52" fill="#141416" stroke="#EDEDEF" strokeOpacity="0.6" />
    <text x="290" y="74" textAnchor="middle" fontFamily={MONO} fontSize="10" fill="#FFB224">resolve</text>
    {[40, 100].map((y) => (
      <g key={y}>
        <path d={`M330 70 C 370 70, 380 ${y}, 420 ${y}`} fill="none" stroke="#FFB224" strokeDasharray="5 5" />
        <rect x="420" y={y - 11} width="92" height="22" fill="#0F0F11" stroke="#36363C" />
        <text x="430" y={y + 4} fontFamily={MONO} fontSize="10" fill="#EDEDEF">{y === 40 ? 'entity + why' : 'link + sources'}</text>
      </g>
    ))}
  </svg>
);

const GraphDiagram: React.FC = () => {
  const nodes = [
    [70, 30], [30, 90], [110, 96], [170, 40], [200, 108],
  ] as const;
  const edges = [[0, 1], [0, 2], [0, 3], [3, 4], [2, 4]] as const;
  return (
    <svg viewBox="0 0 230 140" className="h-full w-full" aria-hidden="true">
      {edges.map(([a, b], i) => (
        <line key={i} x1={nodes[a][0]} y1={nodes[a][1]} x2={nodes[b][0]} y2={nodes[b][1]} stroke={i === 2 ? '#FFB224' : '#6E6E77'} strokeDasharray={i === 2 ? '5 5' : undefined} />
      ))}
      {nodes.map(([x, y], i) => (
        <rect key={i} x={x - 6} y={y - 6} width="12" height="12" fill="#09090A" stroke="#EDEDEF" className="transition-colors duration-base group-hover:stroke-signal-amber" />
      ))}
    </svg>
  );
};

const DashboardDiagram: React.FC = () => (
  <svg viewBox="0 0 230 140" className="h-full w-full" aria-hidden="true">
    <rect x="4" y="4" width="222" height="132" fill="none" stroke="#36363C" />
    <line x1="4" x2="226" y1="24" y2="24" stroke="#36363C" />
    <line x1="80" x2="80" y1="24" y2="136" stroke="#36363C" />
    {[40, 56, 72, 88].map((y) => <rect key={y} x="12" y={y} width="58" height="6" fill="#232327" />)}
    <path d="M92 110 L 120 80 L 150 92 L 180 54 L 214 66" fill="none" stroke="#EDEDEF" strokeOpacity="0.6" strokeDasharray="2 4" />
    {[[120, 80], [180, 54]].map(([x, y]) => <circle key={x} cx={x} cy={y} r="4" fill="#FFB224" />)}
    <text x="12" y="17" fontFamily={MONO} fontSize="8.5" fill="#6E6E77">TIMELINE · MAP · CASE</text>
  </svg>
);

const SequenceDiagram: React.FC = () => (
  <svg viewBox="0 0 230 140" className="h-full w-full" aria-hidden="true">
    <line x1="10" x2="220" y1="70" y2="70" stroke="#36363C" />
    {[24, 66, 108, 132, 176, 206].map((x, i) => (
      <g key={x}>
        <rect x={x - 4} y="66" width="8" height="8" transform={`rotate(45 ${x} 70)`} fill={i === 3 ? '#FF5A4E' : '#EDEDEF'} />
        <line x1={x} x2={x} y1={i % 2 ? 78 : 42} y2={i % 2 ? 98 : 62} stroke="#36363C" />
      </g>
    ))}
    <text x="132" y="118" textAnchor="middle" fontFamily={MONO} fontSize="8.5" fill="#FF5A4E">conflict</text>
    <text x="10" y="30" fontFamily={MONO} fontSize="8.5" fill="#6E6E77">NORMALISED · UTC</text>
  </svg>
);

const PlatformDiagram: React.FC = () => (
  <svg viewBox="0 0 230 140" className="h-full w-full" aria-hidden="true">
    <rect x="4" y="4" width="222" height="132" fill="none" stroke="#36363C" />
    <rect x="4" y="4" width="60" height="132" fill="#0F0F11" stroke="#36363C" />
    <rect x="166" y="4" width="60" height="132" fill="#0F0F11" stroke="#36363C" />
    {[20, 38, 56, 74].map((y, i) => <rect key={y} x="12" y={y} width="44" height="10" fill={i === 1 ? '#FFB224' : '#232327'} />)}
    {[[90, 50], [130, 40], [110, 90], [145, 100]].map(([x, y]) => <rect key={x} x={x - 4} y={y - 4} width="8" height="8" fill="none" stroke="#EDEDEF" />)}
    <line x1="90" y1="50" x2="130" y2="40" stroke="#6E6E77" />
    <line x1="90" y1="50" x2="110" y2="90" stroke="#FFB224" strokeDasharray="4 4" />
    <line x1="110" y1="90" x2="145" y2="100" stroke="#6E6E77" />
    {[20, 34, 48, 62, 76].map((y) => <rect key={y} x="174" y={y} width={y === 20 ? 40 : 30} height="5" fill="#232327" />)}
  </svg>
);

const DIAGRAMS: Record<string, React.FC> = {
  'correlation-engine': CorrelationDiagram,
  'knowledge-graph': GraphDiagram,
  'reconstruction-dashboard': DashboardDiagram,
  'event-sequence': SequenceDiagram,
  'investigation-platform': PlatformDiagram,
};

const DeliverableCell: React.FC<{ item: Deliverable; index: number; wide?: boolean }> = ({ item, index, wide = false }) => {
  const Diagram = DIAGRAMS[item.id];
  return (
    <article
      id={`capability-${item.id}`}
      className={`group flex flex-col justify-between gap-8 bg-ink-950 p-6 transition-colors duration-base hover:bg-ink-900 sm:p-8 ${
        wide ? 'md:col-span-2' : ''
      }`}
    >
      <div className={wide ? 'h-36' : 'h-32'}>{Diagram && <Diagram />}</div>
      <div>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <span className="font-mono text-micro text-fg-subtle">{String(index + 1).padStart(2, '0')}</span>
          <span className={`border px-1.5 py-0.5 font-mono text-micro uppercase ${STATUS_STYLE[item.status]}`}>
            {DELIVERABLE_STATUS_LABEL[item.status]}
          </span>
        </div>
        <h3 className="mt-3 font-display text-2xl font-semibold uppercase leading-none text-fg">{item.title}</h3>
        <p className="mt-3 text-sm leading-relaxed text-fg-muted">{item.description}</p>
        <p className="mt-4 font-mono text-micro uppercase text-fg-subtle">{item.statusNote}</p>
      </div>
    </article>
  );
};

export const DeliverablesSection: React.FC = () => (
  <section id={SECTION_IDS.deliverables} aria-labelledby="deliverables-heading" className="border-b border-ink-line py-section">
    <Container>
      <SectionHeader
        id="deliverables-heading"
        eyebrow="06 — Core deliverables"
        lead="Five capabilities."
        emphasis="One investigation surface."
        aside={
<p>Each labelled with its real status in the repository.</p>
        }
      />
      <Reveal className="mt-14 grid gap-px border border-ink-line bg-ink-line md:grid-cols-2 lg:grid-cols-3">
        {DELIVERABLES.map((item, index) => (
          <DeliverableCell key={item.id} item={item} index={index} wide={index === 0} />
        ))}
      </Reveal>
    </Container>
  </section>
);
