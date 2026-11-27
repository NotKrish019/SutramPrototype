/**
 * The case overview's centrepiece: one large live visual of the reconstruction (network,
 * movement or timeline), drawn with the theatre's scenes at their final frame. Only ambient
 * motion runs (particles along links), and it stops under reduced motion or a hidden tab.
 */
import React, { useEffect, useMemo, useState } from 'react';
import type { CaseDetail } from '../workspace/types';
import { buildTheatreModel } from './model';
import { ClockScene, LinkScene, PlaceScene, STAGE_H, STAGE_W, StageDefs } from './scenes';
import { prefersReducedMotion } from './usePlayhead';

type View = 'network' | 'movement' | 'timeline';

/** Each view is cropped to where its scene draws, so the picture fills the card. */
const VIEWBOX: Record<View, string> = {
  network: '170 85 860 500',
  movement: `30 55 ${STAGE_W - 40} ${STAGE_H - 70}`,
  timeline: `20 40 ${STAGE_W - 20} ${STAGE_H - 50}`,
};

const VIEWS: { key: View; label: string }[] = [
  { key: 'network', label: 'Network' },
  { key: 'movement', label: 'Movement' },
  { key: 'timeline', label: 'Timeline' },
];

/** Seconds since mount at ~30 fps, for ambient loops. Frozen at 0 under reduced motion. */
function useAmbientClock(): number {
  const [t, setT] = useState(0);
  useEffect(() => {
    if (prefersReducedMotion()) return;
    let frame = 0;
    let last = 0;
    const start = performance.now();
    const tick = (now: number): void => {
      if (now - last > 33) {
        last = now;
        setT((now - start) / 1000);
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);
  return t;
}

export const CommandCentre: React.FC<{ detail: CaseDetail; footer?: React.ReactNode }> = ({ detail, footer }) => {
  const model = useMemo(() => buildTheatreModel(detail), [detail]);
  const initial: View = model.nodes.length > 0 ? 'network' : model.places.length > 0 ? 'movement' : 'timeline';
  const [view, setView] = useState<View>(initial);
  const t = useAmbientClock();

  return (
    <div className="cc-card relative overflow-hidden rounded-[1.25rem] border border-white/[0.07]">
      <div className="cc-bg absolute inset-0" aria-hidden="true" />
      <div className="relative flex items-center justify-between gap-3 px-5 pt-4">
        <div role="tablist" aria-label="Reconstruction view" className="inline-flex rounded-full border border-white/10 bg-black/30 p-1 backdrop-blur">
          {VIEWS.map((v) => (
            <button
              key={v.key}
              type="button"
              role="tab"
              aria-selected={view === v.key}
              onClick={() => setView(v.key)}
              className={`h-8 rounded-full px-3.5 text-xs font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40 ${
                view === v.key ? 'bg-white/12 text-fg-bright shadow-[inset_0_0_0_1px_rgba(255,255,255,0.08)]' : 'text-fg-subtle hover:text-fg'
              }`}
            >
              {v.label}
            </button>
          ))}
        </div>
        <p className="hidden text-xs text-fg-subtle sm:block">Solid = observed · dashed = inferred · red = contradiction</p>
      </div>
      <svg
        role="img"
        aria-label={`${VIEWS.find((v) => v.key === view)!.label} view of the reconstruction`}
        viewBox={VIEWBOX[view]}
        preserveAspectRatio="xMidYMid meet"
        className="relative block aspect-[1200/640] w-full"
      >
        <StageDefs />
        {view === 'network' && <LinkScene model={model} p={1} t={t} bare />}
        {view === 'movement' && <PlaceScene model={model} p={1} t={t} />}
        {view === 'timeline' && <ClockScene model={model} p={0.999} t={t} />}
      </svg>
      {footer && <div className="relative border-t border-white/[0.06] bg-black/20 backdrop-blur">{footer}</div>}
    </div>
  );
};
