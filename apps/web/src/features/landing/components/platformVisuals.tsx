import React, { useMemo, useState } from 'react';
import { ArrowDown, ArrowRight, FileText, ShieldCheck } from 'lucide-react';
import { DEMO_CASE, PROVENANCE_LABEL, findEntity, findEvidence } from '../../../fixtures/synthetic/case026';
import { SYNTHETIC_CASE_DETAILS } from '../../../fixtures/synthetic/workspace';
import { Swimlanes } from '../../workspace/pages/EventReconstruction';
import { SitePlan } from '../../workspace/pages/SpatialReconstruction';
import { ProvenanceBadge, ProvenanceLegend, ReviewBadge } from '../../../components/ui/primitives';
import { KnowledgeGraph } from './workbench/KnowledgeGraph';

const MONO = 'JetBrains Mono, ui-monospace, monospace';

/* A — Correlation: three artifacts share signals that propose a link. */
export const CorrelationVisual: React.FC = () => {
  const artifacts = [
    { id: 'EV-01', label: 'CCTV frame', line: ['CAM03 ', 'GATE B', ' 22:41:07'], hit: 1 },
    { id: 'EV-02', label: 'Access log', line: ['badge 4471 ', 'GATE B', ' 22:43:52'], hit: 1 },
    { id: 'EV-05', label: 'Network log', line: ['MAC ', 'D-01', ' AP-WH7 22:44:10'], hit: 1 },
  ];
  return (
    <svg viewBox="0 0 620 340" className="h-auto w-full" role="img" aria-labelledby="corr-title">
      <title id="corr-title">
        Three synthetic artifacts share a location, a time window and a device identifier; the correlation layer
        proposes an inferred link for investigator review.
      </title>
      {artifacts.map((artifact, i) => {
        const y = 24 + i * 104;
        return (
          <g key={artifact.id} fontFamily={MONO}>
            <rect x="8" y={y} width="250" height="78" fill="#0F0F11" stroke="#36363C" />
            <text x="22" y={y + 22} fontSize="9.5" fill="#6E6E77" letterSpacing="0.06em">
              {artifact.id} · {artifact.label.toUpperCase()}
            </text>
            <text x="22" y={y + 50} fontSize="11.5" fill="#A3A3AB">
              {artifact.line.map((part, j) =>
                j === artifact.hit ? (
                  <tspan key={j} fill="#FFB224">{part}</tspan>
                ) : (
                  <tspan key={j}>{part}</tspan>
                ),
              )}
            </text>
            <rect x="22" y={y + 60} width="60" height="3" fill="#232327" />
            <path
              d={`M258 ${y + 39} C 310 ${y + 39}, 320 170, 368 170`}
              fill="none"
              stroke="#FFB224"
              strokeOpacity="0.8"
              strokeWidth="1.25"
              className="flow-line"
              style={{ animationDelay: `${i * -0.6}s` }}
            />
          </g>
        );
      })}
      <g fontFamily={MONO}>
        <rect x="368" y="88" width="244" height="164" fill="#141416" stroke="#EDEDEF" strokeOpacity="0.7" />
        <text x="384" y="112" fontSize="9.5" fill="#FFB224" letterSpacing="0.08em">SHARED SIGNALS</text>
        {[
          ['location', 'Gate B', '#EDEDEF'],
          ['time window', '22:41 – 22:45', '#EDEDEF'],
          ['identifier', 'D-01', '#EDEDEF'],
        ].map(([key, value, color], i) => (
          <g key={key}>
            <line x1="384" x2="596" y1={124 + i * 26} y2={124 + i * 26} stroke="#242428" />
            <text x="384" y={141 + i * 26} fontSize="10.5" fill="#6E6E77">{key}</text>
            <text x="596" y={141 + i * 26} fontSize="10.5" fill={color} textAnchor="end">{value}</text>
          </g>
        ))}
        <line x1="384" x2="596" y1="202" y2="202" stroke="#242428" />
        <line x1="384" x2="410" y1="226" y2="226" stroke="#FFB224" strokeDasharray="5 5" strokeWidth="1.5" />
        <text x="418" y="230" fontSize="10.5" fill="#FFB224">proposed link · inferred</text>
        <text x="384" y="276" fontSize="9.5" fill="#6E6E77">→ queued for investigator review</text>
      </g>
    </svg>
  );
};

/* B — Graph */
export const GraphVisual: React.FC = () => {
  const [selected, setSelected] = useState<string | null>('ENT-PA');
  const neighbours = useMemo(() => {
    if (!selected) return new Set<string>();
    const set = new Set([selected]);
    for (const rel of DEMO_CASE.relationships) {
      if (rel.from === selected) set.add(rel.to);
      if (rel.to === selected) set.add(rel.from);
    }
    return set;
  }, [selected]);
  const entity = selected ? findEntity(selected) : undefined;
  const links = DEMO_CASE.relationships.filter((rel) => rel.from === selected || rel.to === selected);

  return (
    <div className="relative">
      <div className="overflow-x-auto">
        <KnowledgeGraph
          className="min-w-[520px]"
          titleId="platform-graph-title"
          highlighted={neighbours}
          selectedEntityId={selected}
          onSelectEntity={(e) => setSelected((current) => (current === e.id ? null : e.id))}
        />
      </div>
      {entity && (
        <div aria-live="polite" className="mt-3 border border-ink-strong bg-ink-950 p-3">
          <p className="font-mono text-micro uppercase text-fg-subtle">{entity.kind} · tap a node</p>
          <p className="mt-1 text-sm text-fg">{entity.label}</p>
          <ul className="mt-2 grid gap-x-6 gap-y-1 sm:grid-cols-2">
            {links.map((rel) => {
              const other = findEntity(rel.from === selected ? rel.to : rel.from);
              return (
                <li key={rel.id} className="flex items-center justify-between gap-2 text-xs text-fg-muted">
                  <span className="truncate">{rel.label} {other?.label}</span>
                  <span className={`shrink-0 font-mono text-micro uppercase ${rel.provenance === 'inferred' ? 'text-signal-amber' : rel.provenance === 'supplied' ? 'text-signal-supplied' : 'text-fg'}`}>
                    {PROVENANCE_LABEL[rel.provenance]}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      <ProvenanceLegend className="mt-4 border-t border-ink-line pt-4" />
    </div>
  );
};

/* C — Timeline */
const RECON = SYNTHETIC_CASE_DETAILS['CASE-026']!;
const RECON_EVENTS = [...RECON.events].sort((a, b) => a.timestamp.localeCompare(b.timestamp));
const RECON_SEQUENCE = new Map(
  RECON_EVENTS.filter((e) => RECON.places.some((p) => p.event_ids.includes(e.event_id))).map((e, i) => [e.event_id, i + 1]),
);
const RECON_CONFLICTS = RECON.review_items
  .filter((item) => item.category === 'conflicting' && item.related_event_ids.length === 2)
  .map((item) => ({ a: item.related_event_ids[0]!, b: item.related_event_ids[1]!, reviewId: item.id }));

/** C — the workspace's own swimlanes and site plan, driven by one selection. */
export const TimelineVisual: React.FC = () => {
  const [eventId, setEventId] = useState<string | null>('EVT-3');
  const placeId = RECON.places.find((p) => eventId && p.event_ids.includes(eventId))?.place_id ?? null;
  return (
    <div className="space-y-4">
      <div className="overflow-x-auto">
        <Swimlanes
          detail={RECON}
          events={RECON_EVENTS}
          range="focus"
          selectedId={eventId}
          onSelect={setEventId}
          conflicts={RECON_CONFLICTS}
          className="min-w-[34rem]"
        />
      </div>
      <div className="[perspective:1400px]">
        <div className="origin-top border border-ink-line bg-ink-950 transition-transform duration-slow ease-out [transform:rotateX(28deg)] hover:[transform:rotateX(12deg)] motion-reduce:[transform:none]">
          <SitePlan
            places={RECON.places}
            sequence={RECON_SEQUENCE}
            placeId={placeId}
            onSelect={(id) => setEventId(RECON.places.find((p) => p.place_id === id)?.event_ids[0] ?? null)}
            className="min-w-0"
          />
        </div>
      </div>
      <p className="font-mono text-micro uppercase text-fg-subtle">Select an event or place · same component as the workspace</p>
    </div>
  );
};

/* D — Contradiction */
export const ContradictionVisual: React.FC = () => {
  const discrepancy = DEMO_CASE.discrepancies[0];
  if (!discrepancy) return null;
  const records = [discrepancy.recordA, discrepancy.recordB];
  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between gap-3">
        <p className="font-mono text-micro uppercase text-signal-flag">● {discrepancy.title} · {discrepancy.id}</p>
        <ReviewBadge value="pending" />
      </div>
      <div className="grid gap-px bg-ink-line sm:grid-cols-2">
        {records.map((record) => {
          const evidence = findEvidence(record.evidenceId);
          return (
            <div key={record.evidenceId} className="bg-ink-950 p-5">
              <p className="font-mono text-micro uppercase text-fg-subtle">{record.evidenceId} · {evidence?.label}</p>
              <p className="mt-3 font-display text-4xl font-semibold text-fg">{record.value}</p>
              <p className="mt-1 text-sm text-fg-muted">{record.label}</p>
            </div>
          );
        })}
      </div>
      {/* Same gate, two clocks: the gap is annotated, not resolved. */}
      <svg viewBox="0 0 520 90" className="h-auto w-full" role="img" aria-label={`${records[0]!.value} versus ${records[1]!.value}: ${discrepancy.variance}, clock synchronisation requires verification`}>
        <line x1="20" x2="500" y1="52" y2="52" stroke="#36363C" />
        {[0, 1, 2, 3, 4].map((i) => (
          <g key={i}>
            <line x1={20 + i * 120} x2={20 + i * 120} y1="47" y2="57" stroke="#36363C" />
            <text x={20 + i * 120} y="78" textAnchor="middle" fontFamily="JetBrains Mono, monospace" fontSize="10" fill="#6E6E77">
              22:{String(40 + i).padStart(2, '0')}
            </text>
          </g>
        ))}
        <rect x="146" y="44" width="16" height="16" transform="rotate(45 154 52)" fill="#09090A" stroke="#EDEDEF" strokeWidth="1.6" />
        <rect x="476" y="44" width="16" height="16" transform="rotate(45 484 52)" fill="#09090A" stroke="#EDEDEF" strokeWidth="1.6" />
        <path d="M154 30 V 22 H 484 V 30" fill="none" stroke="#FF5A4E" strokeWidth="1.5" />
        <text x="319" y="16" textAnchor="middle" fontFamily="JetBrains Mono, monospace" fontSize="11" fill="#FF5A4E">
          Δ {discrepancy.variance} · verify clock synchronisation
        </text>
      </svg>
      <p className="text-sm text-fg-muted">{discrepancy.summary}</p>
    </div>
  );
};

/* E — Explainability chain */
export const ExplainVisual: React.FC = () => {
  const evidence = findEvidence('EV-05');
  const steps = [
    {
      kind: 'Source artifact',
      icon: FileText,
      title: evidence?.fileName ?? '',
      meta: `SHA-256 ${evidence?.sha256 ?? ''} · recorded at ingestion`,
      badge: null,
    },
    {
      kind: 'Extracted fact',
      icon: ArrowDown,
      title: 'Device D-01 associated with AP-WH7 at 22:44:10',
      meta: 'Line 1,204 of the log export',
      badge: <ProvenanceBadge value="observed" />,
    },
    {
      kind: 'Inferred relationship',
      icon: ArrowRight,
      title: 'Person A — likely user of → Device D-01',
      meta: 'Supported by EV-03 sender metadata and EV-05 association; not proof of identity',
      badge: <ProvenanceBadge value="inferred" />,
    },
  ];
  return (
    <ol className="space-y-0" aria-label="Explainability chain">
      {steps.map((step, index) => (
        <li key={step.kind} className="relative pl-10">
          {index < steps.length - 1 && (
            <span aria-hidden="true" className="absolute bottom-0 left-[0.6875rem] top-8 w-px bg-ink-strong" />
          )}
          <span
            aria-hidden="true"
            className={`absolute left-0 top-1 flex h-6 w-6 items-center justify-center border ${
              index === 2 ? 'border-signal-amber text-signal-amber' : 'border-ink-strong text-fg-muted'
            }`}
          >
            <step.icon className="h-3.5 w-3.5" />
          </span>
          <div className="pb-7">
            <div className="flex flex-wrap items-center gap-2">
              <p className="font-mono text-micro uppercase text-fg-subtle">{step.kind}</p>
              {step.badge}
            </div>
            <p className="mt-2 text-base text-fg">{step.title}</p>
            <p className="mt-1 text-sm text-fg-muted">{step.meta}</p>
          </div>
        </li>
      ))}
      <li className="flex items-center gap-2 border-t border-ink-line pt-4 font-mono text-micro uppercase text-fg-subtle">
        <ShieldCheck aria-hidden="true" className="h-3.5 w-3.5 text-signal-ok" />
        Each step links back to the step before it
      </li>
    </ol>
  );
};
