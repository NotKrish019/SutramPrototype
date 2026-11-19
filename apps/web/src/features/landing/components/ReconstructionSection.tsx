import React, { useState } from 'react';
import { Move3d, Route } from 'lucide-react';
import { SECTION_IDS } from '../data/siteContent';
import { CctvStill } from '../board/artwork/CctvStill';
import { ACCESS_LOG_ROWS, LogPrintout, NETWORK_LOG_ROWS } from '../board/artwork/Documents';
import { EditorialHeading, EvidencePrint, SyntheticStamp } from '../board/materials';
import { WireframeModel } from '../board/wireframe/WireframeModel';
import { MARKERS } from '../board/wireframe/model';
import { useInView } from '../hooks/useInView';

function MarkerEvidence({ id }: { id: string }): React.ReactElement {
  if (id === 'CAM-03')
    return (
      <EvidencePrint rot={-2} tag="EV-01" caption="CCTV-03 · camera clock" imageClassName="aspect-[16/11]">
        <CctvStill annotate />
      </EvidencePrint>
    );
  if (id === 'ACS-B')
    return (
      <div className="-rotate-1">
        <LogPrintout title="Access control log" file="door_controller_export.csv" tag="EV-02" rows={ACCESS_LOG_ROWS} className="shadow-[0_30px_40px_-16px_rgba(0,0,0,.85)]" />
      </div>
    );
  return (
    <div className="rotate-1">
      <LogPrintout title="AP association log" file="warehouse_ap_assoc.log" tag="EV-05" rows={NETWORK_LOG_ROWS} className="shadow-[0_30px_40px_-16px_rgba(0,0,0,.85)]" />
    </div>
  );
}

export const ReconstructionSection: React.FC = () => {
  const [selected, setSelected] = useState<string>('CAM-03');
  const [showRoute, setShowRoute] = useState(true);
  const [revealRef, visible] = useInView<HTMLDivElement>();
  const marker = MARKERS.find((m) => m.id === selected) ?? MARKERS[0]!;

  return (
    <section
      id={SECTION_IDS.reconstruction}
      aria-labelledby="reconstruction-heading"
      className="relative overflow-hidden border-b border-ink-line bg-[radial-gradient(ellipse_at_30%_40%,#121820,#080807_70%)] py-section"
    >
      <div aria-hidden="true" className="pattern-grid pointer-events-none absolute inset-0 opacity-40 [mask-image:radial-gradient(ellipse_at_35%_50%,black,transparent_70%)]" />
      <div className="relative mx-auto max-w-wide px-gutter">
        <EditorialHeading
          id="reconstruction-heading"
          index="03"
          kicker="Spatial reconstruction"
          title="Put every record"
          emphasis="back in its place."
          lead="Sensors, timestamps and movement — mapped onto the site where they were recorded."
        />

        <div ref={revealRef} data-visible={visible} className="reveal mt-14 grid gap-10 lg:grid-cols-12 lg:gap-12">
          {/* 3D stage */}
          <div className="relative lg:col-span-8">
            <div className="relative border border-steel/20 bg-[#07090c]/80 shadow-[0_60px_120px_-40px_rgba(0,0,0,.95),inset_0_1px_0_rgba(143,169,196,.12)]">
              {/* stage chrome */}
              <div className="flex flex-wrap items-center justify-between gap-3 border-b border-steel/15 px-4 py-2.5 font-mono text-[0.625rem] uppercase tracking-[0.14em] text-steel">
                <span>WH-7 · level 0 · reconstruction</span>
                <span className="flex items-center gap-2 text-fg-subtle">
                  <Move3d aria-hidden="true" className="h-3.5 w-3.5" />
                  Drag to rotate
                </span>
              </div>
              <div className="relative">
                {['left-3 top-3 border-l border-t', 'right-3 top-3 border-r border-t', 'left-3 bottom-3 border-l border-b', 'right-3 bottom-3 border-r border-b'].map((c) => (
                  <span key={c} aria-hidden="true" className={`absolute h-5 w-5 border-steel/50 ${c}`} />
                ))}
                <WireframeModel
                  interactive
                  autoRotate
                  showRoute={showRoute}
                  selectedMarker={selected}
                  onSelectMarker={setSelected}
                  label="Synthetic 3D wireframe reconstruction of Warehouse 7 with three sensor locations"
                  className="aspect-[10/7] w-full"
                  initialYaw={-0.55}
                  initialPitch={0.62}
                  distance={58}
                />
                <div aria-hidden="true" className="pointer-events-none absolute bottom-4 left-5 font-mono text-[0.5625rem] uppercase leading-relaxed tracking-[0.14em] text-steel/80">
                  <p>grid 4 m · height 4.2 m</p>
                  <p>{showRoute ? 'route: inferred from sensor records' : 'route hidden'}</p>
                </div>
              </div>
            </div>
          </div>

          {/* Sensor dossier */}
          <div className="lg:col-span-4">
            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                aria-pressed={showRoute}
                onClick={() => setShowRoute((v) => !v)}
                className={`inline-flex items-center gap-2 border px-4 py-2.5 font-mono text-[0.6875rem] uppercase tracking-[0.12em] transition-colors duration-base focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson ${
                  showRoute ? 'border-crimson bg-crimson/15 text-fg-bright' : 'border-paper/30 text-fg hover:border-paper/70'
                }`}
              >
                <Route aria-hidden="true" className="h-4 w-4" />
                Reconstructed route
              </button>
              <SyntheticStamp />
            </div>

            <p className="mt-8 font-mono text-[0.625rem] uppercase tracking-[0.16em] text-fg-subtle">Sensors on site</p>
            <ul className="mt-3 divide-y divide-ink-line border-y border-ink-line">
              {MARKERS.map((m) => {
                const active = m.id === selected;
                return (
                  <li key={m.id}>
                    <button
                      type="button"
                      aria-pressed={active}
                      onClick={() => setSelected(m.id)}
                      className={`group flex w-full items-center justify-between gap-4 py-3.5 text-left transition-colors focus:outline-none focus-visible:bg-paper/5 ${
                        active ? 'text-fg-bright' : 'text-fg-muted hover:text-fg'
                      }`}
                    >
                      <span className="flex items-center gap-3">
                        <span aria-hidden="true" className={`h-2 w-2 rounded-full ring-2 transition-colors ${active ? 'bg-crimson ring-crimson/30' : 'bg-transparent ring-steel/60'}`} />
                        <span>
                          <span className="block text-sm">{m.label}</span>
                          <span className="block font-mono text-[0.625rem] uppercase tracking-[0.1em] text-fg-subtle">{m.evidenceId}</span>
                        </span>
                      </span>
                      <span className={`font-mono text-sm ${active ? 'text-crimson' : 'text-fg-subtle'}`}>{m.time}</span>
                    </button>
                  </li>
                );
              })}
            </ul>

            <div aria-live="polite" className="mt-10">
              <div className="mx-auto max-w-[20rem] lg:mx-0">
                <MarkerEvidence id={marker.id} />
              </div>
              <p className="mt-6 font-serif text-lg leading-snug text-fg-bright">{marker.summary}</p>
              <p className="mt-2 font-mono text-[0.625rem] uppercase tracking-[0.12em] text-fg-subtle">
                {marker.evidenceId} · observed at {marker.time} · {marker.id}
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
