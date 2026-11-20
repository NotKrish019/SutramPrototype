import React, { useEffect, useState } from 'react';
import { Boxes, CalendarRange, Fingerprint as FingerprintIcon, GitFork, Layers, TriangleAlert } from 'lucide-react';
import { ROUTES } from '../../../router/router';
import { CtaLink } from '../../../components/ui/primitives';
import { CAPABILITY_STRIP, SECTION_IDS } from '../data/siteContent';
import { CctvStill } from '../board/artwork/CctvStill';
import { Fingerprint } from '../board/artwork/Fingerprint';
import { MapEvidence } from '../board/artwork/MapEvidence';
import { PhoneEvidence } from '../board/artwork/PhoneEvidence';
import { useMediaQuery, usePointerParallax, useReducedMotion } from '../board/hooks';
import { EvidencePrint, Pushpin, Thread, sag } from '../board/materials';
import { WireframeModel } from '../board/wireframe/WireframeModel';

const STRIP_ICONS: Record<string, typeof Layers> = {
  'multi-source': Layers,
  correlation: GitFork,
  spatial: Boxes,
  timeline: CalendarRange,
  contradiction: TriangleAlert,
  provenance: FingerprintIcon,
};

/** Board coordinates for the desktop composition, in % of the hero box. */
const ITEMS = {
  cctv: { left: 3.2, top: 15, width: 17.5, rot: -5 },
  finger: { left: 1.6, top: 40.5, width: 11.5, rot: 4 },
  phone: { left: 13.4, top: 42, width: 8.4, rot: 7 },
  map: { left: 4, top: 65, width: 15.5, rot: -3 },
  recon: { left: 68.2, top: 15.5, width: 29.8, rot: 2.4 },
} as const;

type ItemKey = keyof typeof ITEMS;

/** Pin location of an item in the 1440×900 thread viewBox. */
function pin(key: ItemKey): [number, number] {
  const item = ITEMS[key];
  return [((item.left + item.width / 2) / 100) * 1440, (item.top / 100) * 900 - 6];
}

const THREADS: { d: string; delay: number }[] = (() => {
  const [cx, cy] = pin('cctv');
  const [fx, fy] = pin('finger');
  const [px, py] = pin('phone');
  const [mx, my] = pin('map');
  const [rx, ry] = pin('recon');
  return [
    { d: sag(-20, 70, cx, cy, 0.05), delay: 0 },
    { d: sag(cx, cy, rx, ry, 0.045), delay: 200 },
    { d: sag(fx, fy, cx, cy, 0.08), delay: 380 },
    { d: sag(fx, fy, px, py, 0.12), delay: 520 },
    { d: sag(px, py, mx, my, 0.1), delay: 660 },
    { d: sag(rx, ry, 1460, 40, 0.06), delay: 800 },
    { d: sag(mx, my, -20, 860, 0.05), delay: 940 },
  ];
})();

/** Prints drop onto the board on load, then the threads draw between them. */
function useBoardEntrance(): { settled: boolean; drawn: boolean } {
  const reduced = useReducedMotion();
  const [settled, setSettled] = useState(reduced);
  const [drawn, setDrawn] = useState(reduced);
  useEffect(() => {
    if (reduced) {
      setSettled(true);
      setDrawn(true);
      return;
    }
    const a = window.setTimeout(() => setSettled(true), 60);
    const b = window.setTimeout(() => setDrawn(true), 950);
    return () => {
      window.clearTimeout(a);
      window.clearTimeout(b);
    };
  }, [reduced]);
  return { settled, drawn };
}

const HeroCopy: React.FC = () => (
  <div className="relative mx-auto flex max-w-[40rem] flex-col items-center text-center">
    <p className="mb-6 font-mono text-[0.6875rem] uppercase tracking-[0.22em] text-fg-subtle">
      Digital evidence intelligence <span className="text-crimson">/</span> crime reconstruction
    </p>
    <h1 aria-describedby="hero-lead" className="flex flex-col items-center">
      {/* Wordmark inside a red viewfinder, as on an evidence monitor. */}
      <span className="relative inline-block px-[0.42em] py-[0.12em]">
        <span aria-hidden="true" className="absolute left-0 top-0 h-[22%] w-[9%] border-l-2 border-t-2 border-crimson" />
        <span aria-hidden="true" className="absolute right-0 top-0 h-[22%] w-[9%] border-r-2 border-t-2 border-crimson" />
        <span aria-hidden="true" className="absolute bottom-0 left-0 h-[22%] w-[9%] border-b-2 border-l-2 border-crimson" />
        <span aria-hidden="true" className="absolute bottom-0 right-0 h-[22%] w-[9%] border-b-2 border-r-2 border-crimson" />
        <span className="serif-tight block font-serif text-[clamp(4.5rem,11vw,9.5rem)] font-semibold leading-[0.9] tracking-[0.02em] text-fg-bright [text-shadow:0_2px_0_rgba(0,0,0,.6),0_24px_60px_rgba(0,0,0,.8)]">
          YUKTI
        </span>
      </span>
      <svg aria-hidden="true" viewBox="0 0 420 22" className="-mt-1 h-4 w-[min(78%,26rem)] overflow-visible" preserveAspectRatio="none">
        <defs>
          <filter id="hero-brush">
            <feTurbulence type="fractalNoise" baseFrequency="0.04 0.9" numOctaves="2" seed="3" />
            <feDisplacementMap in="SourceGraphic" scale="6" />
          </filter>
        </defs>
        <path d="M6 13 C 120 6, 300 6, 414 11" pathLength={1} stroke="#E5243B" strokeWidth="7" strokeLinecap="round" fill="none" filter="url(#hero-brush)" className="brush-stroke" />
      </svg>
      <span className="mt-8 block font-serif text-[clamp(1.4rem,2.1vw,2.1rem)] font-light leading-[1.18] text-fg-bright">
        Every fragment tells a story.{' '}
        <em className="block italic text-crimson">Every connection changes the investigation.</em>
      </span>
    </h1>
    <p id="hero-lead" className="mt-6 max-w-md text-[0.95rem] leading-relaxed text-fg-muted">
      YUKTI connects digital evidence across sources to reconstruct events and reveal traceable investigative intelligence.
    </p>
    <div className="mt-9 flex flex-col gap-3 sm:flex-row">
      <CtaLink href={`#${SECTION_IDS.capabilities}`} arrow>
        Explore the Platform
      </CtaLink>
      <CtaLink href={ROUTES.workspace} variant="secondary">
        Enter Workspace
      </CtaLink>
    </div>
  </div>
);

const CapabilityStrip: React.FC<{ className?: string }> = ({ className = '' }) => (
  <nav aria-label="Capabilities" className={`stitched border border-ink-strong bg-[#0d0c0b]/90 shadow-[0_30px_60px_-20px_rgba(0,0,0,.9)] ${className}`}>
    <ul className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6">
      {CAPABILITY_STRIP.map((item, index) => {
        const Icon = STRIP_ICONS[item.id] ?? Layers;
        return (
          <li key={item.id} className={`${index > 0 ? 'lg:border-l' : ''} border-ink-line`}>
            <a
              href={item.href}
              className="group flex h-full flex-col items-center gap-2.5 px-3 py-4 text-center transition-colors duration-base hover:bg-paper/[0.04] focus:outline-none focus-visible:bg-paper/[0.06]"
            >
              <Icon
                aria-hidden="true"
                strokeWidth={1.4}
                className={`h-6 w-6 transition-colors duration-base ${item.id === 'contradiction' ? 'text-crimson' : 'text-fg group-hover:text-crimson'}`}
              />
              <span className="text-[0.8125rem] leading-tight text-fg-muted transition-colors group-hover:text-fg-bright">{item.label}</span>
            </a>
          </li>
        );
      })}
    </ul>
  </nav>
);

const ReconPrint: React.FC<{ active?: boolean }> = () => (
  <EvidencePrint rot={0} pin="crimson" tag="RECON-WH7" caption="Spatial reconstruction · synthetic" imageClassName="aspect-[4/3] bg-[radial-gradient(circle_at_50%_45%,#18202a,#07090b_75%)]">
    <WireframeModel autoRotate showLabels={false} label="Rotating wireframe reconstruction of a synthetic warehouse with a red movement route" initialYaw={-0.9} initialPitch={0.66} />
    <div className="pointer-events-none absolute left-3 top-2.5 font-mono text-[0.5625rem] uppercase tracking-[0.14em] text-steel">
      WH-7 · L0 · route reconstructed
    </div>
  </EvidencePrint>
);

/* ---------------------------------------------------------------- Desktop */

const DesktopBoard: React.FC = () => {
  const ref = usePointerParallax<HTMLDivElement>();
  const { settled, drawn } = useBoardEntrance();
  const place = (key: ItemKey, depth: number, delay: number): React.CSSProperties =>
    ({
      left: `${ITEMS[key].left}%`,
      top: `${ITEMS[key].top}%`,
      width: `${ITEMS[key].width}%`,
      '--depth': depth,
      '--settle-delay': `${delay}ms`,
      '--settle-rot': `${ITEMS[key].rot - 8}deg`,
    }) as React.CSSProperties;

  return (
    <div ref={ref} className="scene-3d relative mx-auto h-[clamp(760px,100svh,62.5vw)] max-h-[1080px] w-full">
      {/* out-of-focus documents at the edges */}
      <div aria-hidden="true" className="parallax-layer absolute inset-0" style={{ '--depth': -6 } as React.CSSProperties}>
        <div className="paper-scrap left-[34%] top-[-4%] h-[16%] w-[13%] rotate-[8deg]" />
        <div className="paper-scrap left-[56%] top-[2%] h-[12%] w-[10%] -rotate-6" />
        <div className="paper-scrap bottom-[-6%] right-[-2%] h-[30%] w-[22%] rotate-[-14deg]" />
        <div className="paper-scrap bottom-[-8%] left-[-3%] h-[20%] w-[16%] rotate-12" />
      </div>

      <svg
        aria-hidden="true"
        viewBox="0 0 1440 900"
        preserveAspectRatio="none"
        className="parallax-layer pointer-events-none absolute inset-0 z-0 h-full w-full"
        style={{ '--depth': 8 } as React.CSSProperties}
        data-drawn={drawn}
      >
        {THREADS.map((thread) => (
          <Thread key={thread.d} d={thread.d} delay={thread.delay} />
        ))}
      </svg>

      <div className="parallax-layer absolute inset-0 preserve-3d" style={{ '--depth': 14 } as React.CSSProperties}>
        <div className="settle absolute z-20" data-settled={settled} style={place('cctv', 14, 0)}>
          <EvidencePrint rot={ITEMS.cctv.rot} tag="CCTV-03" caption="Gate B · 22:41:07" imageClassName="aspect-[16/11]">
            <CctvStill />
          </EvidencePrint>
        </div>
        <div className="settle absolute z-10" data-settled={settled} style={place('finger', 14, 140)}>
          <EvidencePrint rot={ITEMS.finger.rot} pin="steel" tag="FP-014" caption="Latent" imageClassName="aspect-[5/6] bg-[#08090a]">
            <Fingerprint variant="scan" minutiae label="Synthetic fingerprint scan with four marked minutiae" />
          </EvidencePrint>
        </div>
        <div className="settle absolute z-30" data-settled={settled} style={place('phone', 14, 260)}>
          <div className="is-liftable relative" style={{ transform: `rotate(${ITEMS.phone.rot}deg)` }}>
            <Pushpin className="-top-3 left-1/2 -translate-x-1/2" />
            <PhoneEvidence compact className="transition-transform duration-slow ease-out hover:-translate-y-2" />
          </div>
        </div>
        <div className="settle absolute z-20" data-settled={settled} style={place('map', 14, 380)}>
          <EvidencePrint rot={ITEMS.map.rot} tag="LOC-07" caption="Route · 21:58 → 22:44" imageClassName="aspect-[16/11]">
            <MapEvidence label={false} />
          </EvidencePrint>
        </div>
      </div>

      <div className="parallax-layer absolute inset-0 preserve-3d" style={{ '--depth': 10 } as React.CSSProperties}>
        <div className="settle absolute z-20" data-settled={settled} style={place('recon', 10, 200)}>
          <div style={{ transform: `rotate(${ITEMS.recon.rot}deg)` }}>
            <ReconPrint />
          </div>
        </div>
        <p className="absolute right-[3%] top-[71%] max-w-[15rem] -rotate-1 border-l border-crimson/70 pl-3 font-mono text-[0.625rem] uppercase leading-relaxed tracking-[0.12em] text-fg-subtle">
          Route inferred from three sensor records — every point traceable to its <span className="text-paper">source</span>.
        </p>
      </div>

      {/* documentary details */}
      <div aria-hidden="true" className="absolute left-[3.2%] top-[10%] font-mono text-[0.5625rem] uppercase tracking-[0.18em] text-fg-subtle">
        CASE-026 / board 01 / synthetic
      </div>

      <div className="absolute inset-x-0 top-[13%] z-30 px-4">
        <HeroCopy />
      </div>

      <div className="absolute inset-x-0 bottom-[4%] z-30 flex justify-center px-4">
        <CapabilityStrip className="w-[min(60%,58rem)]" />
      </div>
    </div>
  );
};

/* ---------------------------------------------------------- Tablet/mobile */

const CompactBoard: React.FC = () => {
  const { settled, drawn } = useBoardEntrance();
  return (
    <div className="relative px-gutter pb-16 pt-28 sm:pt-32">
      <HeroCopy />

      <div className="relative mx-auto mt-16 max-w-xl">
        <svg aria-hidden="true" viewBox="0 0 400 520" preserveAspectRatio="none" className="pointer-events-none absolute inset-0 z-0 h-full w-full" data-drawn={drawn}>
          <Thread d={sag(110, 8, 300, 22, 0.08)} />
          <Thread d={sag(300, 22, 96, 196, 0.06)} delay={200} />
          <Thread d={sag(96, 196, 296, 214, 0.1)} delay={400} />
          <Thread d={sag(296, 214, 200, 330, 0.08)} delay={600} />
        </svg>
        <div className="relative z-10 grid grid-cols-2 gap-x-5 gap-y-10">
          <div className="settle" data-settled={settled}>
            <EvidencePrint rot={-4} tag="CCTV-03" imageClassName="aspect-[16/11]">
              <CctvStill />
            </EvidencePrint>
          </div>
          <div className="settle pt-4" data-settled={settled} style={{ '--settle-delay': '120ms' } as React.CSSProperties}>
            <EvidencePrint rot={3} pin="steel" tag="FP-014" imageClassName="aspect-[16/11] bg-[#08090a]">
              <Fingerprint variant="scan" minutiae label="Synthetic fingerprint scan" />
            </EvidencePrint>
          </div>
          <div className="settle" data-settled={settled} style={{ '--settle-delay': '240ms' } as React.CSSProperties}>
            <EvidencePrint rot={2} tag="LOC-07" imageClassName="aspect-[16/11]">
              <MapEvidence label={false} />
            </EvidencePrint>
          </div>
          <div className="settle flex justify-center" data-settled={settled} style={{ '--settle-delay': '360ms' } as React.CSSProperties}>
            <div className="relative w-[46%] rotate-6">
              <Pushpin className="-top-3 left-1/2 -translate-x-1/2" />
              <PhoneEvidence compact />
            </div>
          </div>
        </div>
        <div className="settle relative z-10 mt-10" data-settled={settled} style={{ '--settle-delay': '480ms' } as React.CSSProperties}>
          <ReconPrint />
        </div>
      </div>

      <CapabilityStrip className="mx-auto mt-14 max-w-xl" />
    </div>
  );
};

export const HeroBoard: React.FC = () => {
  const desktop = useMediaQuery('(min-width: 1024px)', true);
  return (
    <section aria-label="Introduction" className="board-surface relative overflow-hidden border-b border-ink-line">
      <div aria-hidden="true" className="board-vignette" />
      {desktop ? <DesktopBoard /> : <CompactBoard />}
    </section>
  );
};
