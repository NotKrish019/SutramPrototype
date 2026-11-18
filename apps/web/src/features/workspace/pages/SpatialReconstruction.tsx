import React from 'react';
import { ArrowRight, MapPinOff } from 'lucide-react';
import { Link } from '../../../router/Link';
import { caseHref } from '../../../router/router';
import { useCaseIndex, useQuerySelection } from '../caseIndex';
import { KeyValueList, Panel } from '../components/ui';
import { formatEventTime } from '../format';
import type { CaseDetail, PlaceKind, SpatialPlace } from '../types';

const MONO = 'JetBrains Mono, ui-monospace, monospace';

const KIND_LABEL: Record<PlaceKind, string> = {
  site: 'Site',
  gate: 'Gate',
  camera: 'Camera',
  'access-point': 'Wi-Fi access point',
  'off-plan': 'Off-plan location',
};

const PlaceGlyph: React.FC<{ place: SpatialPlace; selected: boolean }> = ({ place, selected }) => {
  const stroke = selected ? '#FFB224' : '#EDEDEF';
  const fill = selected ? '#2a2112' : '#0F0F11';
  const { x, y } = place;
  switch (place.kind) {
    case 'gate':
      return <rect x={x - 7} y={y - 22} width="14" height="44" fill={fill} stroke={stroke} strokeWidth="1.6" />;
    case 'camera':
      return <rect x={x - 8} y={y - 6} width="16" height="12" fill={fill} stroke={stroke} strokeWidth="1.6" />;
    case 'access-point':
      return <circle cx={x} cy={y} r="8" fill={fill} stroke={stroke} strokeWidth="1.6" />;
    case 'off-plan':
      return <rect x={x - 9} y={y - 9} width="18" height="18" transform={`rotate(45 ${x} ${y})`} fill={fill} stroke={stroke} strokeWidth="1.6" strokeDasharray="3 2" />;
    case 'site':
    default:
      return <rect x={x - 6} y={y - 6} width="12" height="12" fill={stroke} />;
  }
};

export interface SitePlanProps {
  places: SpatialPlace[];
  /** Event order (1-based) by normalised time, keyed by event id. */
  sequence: Map<string, number>;
  placeId: string | null;
  onSelect: (placeId: string | null) => void;
  className?: string;
}

/** Schematic site plan (not geographic). Shared by the workspace and the public site. */
export const SitePlan: React.FC<SitePlanProps> = ({ places, sequence, placeId, onSelect, className = 'min-w-[36rem]' }) => {
  const ap = places.find((p) => p.kind === 'access-point');
  const camera = places.find((p) => p.kind === 'camera');
  const gate = places.find((p) => p.kind === 'gate');
  const offPlan = places.find((p) => p.kind === 'off-plan');
  return (
    <svg viewBox="0 0 800 520" className={`block h-auto w-full ${className}`} role="group" aria-label="Schematic site plan of Warehouse 7 with evidence-linked places">
      <defs>
        <pattern id="bldg-hatch" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <line x1="0" y1="0" x2="0" y2="8" stroke="#EDEDEF" strokeOpacity="0.06" strokeWidth="3" />
        </pattern>
        <radialGradient id="ap-coverage">
          <stop offset="0%" stopColor="#FFB224" stopOpacity="0.14" />
          <stop offset="100%" stopColor="#FFB224" stopOpacity="0.02" />
        </radialGradient>
      </defs>
      {/* road + boundary */}
      <rect x="40" y="20" width="80" height="480" fill="#141416" />
      <text x="80" y="260" textAnchor="middle" fontFamily={MONO} fontSize="10" fill="#6E6E77" transform="rotate(-90 80 260)">SERVICE ROAD</text>
      <rect x="160" y="60" width="600" height="400" fill="none" stroke="#36363C" strokeDasharray="6 5" />
      <text x="752" y="450" textAnchor="end" fontFamily={MONO} fontSize="10" fill="#6E6E77">SITE BOUNDARY</text>
      <rect x="340" y="140" width="300" height="220" fill="url(#bldg-hatch)" stroke="#A3A3AB" strokeOpacity="0.6" />
      <text x="490" y="380" textAnchor="middle" fontFamily={MONO} fontSize="11" fill="#A3A3AB">WAREHOUSE 7</text>
      {gate && <line x1="120" x2={gate.x - 7} y1={gate.y} y2={gate.y} stroke="#36363C" strokeDasharray="2 4" />}

      {ap?.uncertainty_radius && (
        <g>
          <circle cx={ap.x} cy={ap.y} r={ap.uncertainty_radius} fill="url(#ap-coverage)" stroke="#FFB224" strokeOpacity="0.5" strokeDasharray="4 4" />
          <text x={ap.x} y={ap.y - ap.uncertainty_radius - 8} textAnchor="middle" fontFamily={MONO} fontSize="10" fill="#FFB224" className="svg-label">
            AP-WH7 COVERAGE · DEVICE SOMEWHERE WITHIN
          </text>
        </g>
      )}
      {camera && gate && (
        <path d={`M${camera.x} ${camera.y} L ${gate.x + 70} ${gate.y - 30} L ${gate.x + 20} ${gate.y + 50} Z`} fill="#7DB4FF" fillOpacity="0.07" stroke="#7DB4FF" strokeOpacity="0.4" strokeDasharray="3 3" />
      )}
      {offPlan && (
        <g>
          <line x1="640" y1="140" x2={offPlan.x - 14} y2={offPlan.y + 14} stroke="#6E6E77" strokeDasharray="2 5" />
          <text x={offPlan.x - 22} y={offPlan.y + 4} textAnchor="end" fontFamily={MONO} fontSize="10" fill="#6E6E77">OFF-PLAN · NOT TO SCALE</text>
        </g>
      )}

      {places
        .filter((p) => p.kind !== 'site')
        .map((place) => {
          const isSelected = place.place_id === placeId;
          const seq = place.event_ids.map((id) => sequence.get(id)).filter((n): n is number => n !== undefined);
          return (
            <g
              key={place.place_id}
              role="button"
              tabIndex={0}
              aria-pressed={isSelected}
              aria-label={`${place.name}, ${KIND_LABEL[place.kind]}, ${place.event_ids.length} linked events`}
              onClick={() => onSelect(isSelected ? null : place.place_id)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onSelect(isSelected ? null : place.place_id);
                }
              }}
              className="cursor-pointer focus:outline-none [&:focus-visible_.ring]:opacity-100"
            >
              <circle className="ring" cx={place.x} cy={place.y} r="30" fill="none" stroke="#FFB224" strokeDasharray="3 3" opacity="0" />
              <rect x={place.x - 34} y={place.y - 30} width="68" height="74" fill="transparent" />
              <PlaceGlyph place={place} selected={isSelected} />
              <text x={place.x + (place.kind === 'gate' ? 14 : 0)} y={place.y + (place.kind === 'gate' ? 4 : 26)} textAnchor={place.kind === 'gate' ? 'start' : 'middle'} fontFamily={MONO} fontSize="11" fill="#EDEDEF" className="svg-label">
                {place.name}
              </text>
              {seq.map((n, i) => (
                <g key={n}>
                  <rect x={place.x + 12 + i * 18} y={place.y - 30} width="16" height="16" fill="#09090A" stroke={isSelected ? '#FFB224' : '#A3A3AB'} />
                  <text x={place.x + 20 + i * 18} y={place.y - 18} textAnchor="middle" fontFamily={MONO} fontSize="10" fill="#EDEDEF">{n}</text>
                </g>
              ))}
            </g>
          );
        })}
    </svg>
  );
};

export const SpatialReconstruction: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const index = useCaseIndex(detail);
  const caseId = detail.case.case_id;
  const [placeId, setPlaceId] = useQuerySelection('place');
  const places = detail.places;
  const selected = places.find((p) => p.place_id === placeId) ?? null;

  // Sequence numbers follow normalised time across all located events.
  const located = new Set(places.flatMap((p) => p.event_ids));
  const ordered = [...detail.events].filter((e) => located.has(e.event_id)).sort((a, b) => a.timestamp.localeCompare(b.timestamp));
  const sequence = new Map(ordered.map((e, i) => [e.event_id, i + 1]));
  const unlocated = detail.events.filter((e) => !located.has(e.event_id));

  if (places.length === 0) {
    return (
      <div className="p-4 sm:p-6 lg:p-8">
        <Panel title="Spatial reconstruction">
          <div className="flex flex-col items-start gap-3 px-5 py-10">
            <MapPinOff aria-hidden="true" className="h-5 w-5 text-fg-subtle" />
            <p className="text-sm text-fg-muted">No location records are available for this case.</p>
          </div>
        </Panel>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 gap-4 p-4 sm:p-6 lg:p-8 xl:grid-cols-12">
      <div className="min-w-0 space-y-4 xl:col-span-8">
        <Panel title="Site plan · Warehouse 7" meta="schematic · not to scale · not geographic">
          <div className="overflow-x-auto bg-ink-950">
            <SitePlan places={places} sequence={sequence} placeId={placeId} onSelect={setPlaceId} />
          </div>
          <ul className="grid gap-x-6 gap-y-2 border-t border-ink-line px-4 py-3 font-mono text-micro uppercase text-fg-muted sm:grid-cols-2 xl:grid-cols-4">
            <li className="flex items-center gap-2"><span aria-hidden="true" className="inline-flex h-4 w-4 items-center justify-center border border-fg-muted text-[0.6rem] text-fg">1</span> Event order (normalised)</li>
            <li className="flex items-center gap-2"><span aria-hidden="true" className="h-3 w-5 border border-dashed border-signal-amber/70 bg-signal-amber/10" /> Area of uncertainty</li>
            <li className="flex items-center gap-2"><span aria-hidden="true" className="h-3 w-5 border border-dashed border-signal-supplied/60 bg-signal-supplied/10" /> Approx. camera view</li>
            <li className="flex items-center gap-2"><span aria-hidden="true" className="h-3 w-3 rotate-45 border border-dashed border-fg-muted" /> Off-plan place</li>
          </ul>
        </Panel>

        <Panel title="Events without a location">
          {unlocated.length === 0 ? (
            <p className="px-4 py-3 text-sm text-fg-subtle">All events are linked to a place.</p>
          ) : (
            <ul className="divide-y divide-ink-line">
              {unlocated.map((event) => (
                <li key={event.event_id}>
                  <Link to={caseHref(caseId, 'timeline', { event: event.event_id })} className="flex items-center justify-between gap-3 px-4 py-2.5 hover:bg-white/[0.02]">
                    <span className="text-sm text-fg">{event.title}</span>
                    <span className="font-mono text-micro text-fg-subtle">{formatEventTime(event.timestamp, event.timestamp_precision)} · no location data</span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
          <p className="border-t border-ink-line px-4 py-2.5 text-xs text-fg-subtle">
            No GPS or cell-site records are in this case, so movement between places is not reconstructed.
          </p>
        </Panel>
      </div>

      <div className="xl:col-span-4 xl:self-start">
        {selected ? (
          <Panel as="aside" title={`${KIND_LABEL[selected.kind]} · ${selected.place_id}`}>
            <div className="border-b border-ink-line px-4 py-4">
              <p className="text-lg text-fg">{selected.name}</p>
              <p className="mt-1 text-sm leading-relaxed text-fg-muted">{selected.note}</p>
            </div>
            <KeyValueList
              items={[
                ['Kind', KIND_LABEL[selected.kind]],
                ['Coordinates', 'Schematic only — no geographic coordinates recorded'],
                ...(selected.uncertainty_radius ? ([['Precision', 'Area (radio coverage), not a point']] as [string, string][]) : []),
              ]}
            />
            <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle">Linked events</h3>
            {selected.event_ids.length === 0 ? (
              <p className="px-4 py-3 text-sm text-fg-subtle">None.</p>
            ) : (
              <ul className="divide-y divide-ink-line">
                {selected.event_ids.map((id) => {
                  const event = index.events.get(id);
                  return (
                    <li key={id}>
                      <Link to={caseHref(caseId, 'timeline', { event: id })} className="flex items-center justify-between gap-3 px-4 py-2.5 hover:bg-white/[0.02]">
                        <span className="min-w-0">
                          <span className="block truncate text-sm text-fg">{sequence.get(id)}. {event?.title}</span>
                          <span className="font-mono text-micro text-fg-subtle">{event ? `${formatEventTime(event.timestamp, event.timestamp_precision)} IST` : id}</span>
                        </span>
                        <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />
                      </Link>
                    </li>
                  );
                })}
              </ul>
            )}
            <h3 className="border-y border-ink-line bg-ink-850 px-4 py-2 font-mono text-micro uppercase text-fg-subtle">Evidence</h3>
            <ul className="divide-y divide-ink-line">
              {selected.evidence_ids.map((id) => (
                <li key={id}>
                  <Link to={caseHref(caseId, 'evidence', { evidence: id })} className="flex items-center justify-between px-4 py-2.5 text-sm text-fg hover:bg-white/[0.02]">
                    <span className="truncate">{id} · <span className="text-fg-muted">{index.evidence.get(id)?.original_filename}</span></span>
                    <ArrowRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />
                  </Link>
                </li>
              ))}
            </ul>
          </Panel>
        ) : (
          <Panel title="Location detail">
            <p className="px-4 py-6 text-sm leading-relaxed text-fg-muted">
              Select a place on the plan to see its linked events and evidence. Numbers show the order of events by
              normalised time.
            </p>
          </Panel>
        )}
      </div>
    </div>
  );
};
