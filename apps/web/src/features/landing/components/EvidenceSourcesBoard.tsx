import React from 'react';
import { SECTION_IDS } from '../data/siteContent';
import { useInView } from '../hooks/useInView';
import { CctvStill } from '../board/artwork/CctvStill';
import { CommsDocument, LogPrintout, NETWORK_LOG_ROWS, TransactionSlip } from '../board/artwork/Documents';
import { Fingerprint } from '../board/artwork/Fingerprint';
import { MapEvidence } from '../board/artwork/MapEvidence';
import { PhoneEvidence } from '../board/artwork/PhoneEvidence';
import { EditorialHeading, EvidencePrint, Pushpin, SyntheticStamp } from '../board/materials';
import { ThreadCanvas, type ThreadLink } from '../board/ThreadCanvas';

interface SourceArtifact {
  id: string;
  index: string;
  title: string;
  line: string;
  tag: string;
  meta: string;
  /** Desktop placement, % of the board. */
  x: number;
  y: number;
  w: number;
  rot: number;
  render: () => React.ReactNode;
}

const SOURCES: SourceArtifact[] = [
  {
    id: 'cctv',
    index: '01',
    title: 'CCTV & video',
    line: 'Frames, cameras and burned-in clocks — held uncertain until verified.',
    tag: 'EV-01',
    meta: 'gateB_cam03.mp4 · sha256 9f2c…a41e',
    x: 1.5,
    y: 3,
    w: 25,
    rot: -3,
    render: () => (
      <EvidencePrint rot={0} pinId="src-cctv" tag="CCTV-03" caption="Gate B · camera clock" imageClassName="aspect-[16/11]">
        <CctvStill annotate />
      </EvidencePrint>
    ),
  },
  {
    id: 'mobile',
    index: '02',
    title: 'Mobile devices',
    line: 'Messages, calls, app data and device locations.',
    tag: 'EV-03',
    meta: 'device_D01_chat_export.json',
    x: 32,
    y: 0,
    w: 11.5,
    rot: 4,
    render: () => (
      <div className="relative">
        <Pushpin pinId="src-mobile" className="-top-3 left-1/2 -translate-x-1/2" />
        <PhoneEvidence />
      </div>
    ),
  },
  {
    id: 'fingerprint',
    index: '03',
    title: 'Fingerprint & forensic records',
    line: 'Lifts and lab records kept as artifacts — comparison is never identity.',
    tag: 'FP-014',
    meta: 'latent_lift_014.tif · comparison pending',
    x: 49,
    y: 5,
    w: 16,
    rot: -2.5,
    render: () => (
      <EvidencePrint rot={0} pin="steel" pinId="src-fingerprint" tag="FP-014" caption="Latent lift" imageClassName="aspect-[5/6] bg-[#ebdfc8] [background-image:linear-gradient(rgba(43,34,24,.07)_1px,transparent_1px),linear-gradient(90deg,rgba(43,34,24,.07)_1px,transparent_1px)] [background-size:12px_12px]">
        <Fingerprint variant="ink" label="Synthetic inked fingerprint lift" className="p-3" />
      </EvidencePrint>
    ),
  },
  {
    id: 'comms',
    index: '04',
    title: 'Communication records',
    line: 'Headers, threads and participants across channels.',
    tag: 'EV-06',
    meta: 'mail_export.mbox · headers only',
    x: 72,
    y: 1,
    w: 22,
    rot: 3,
    render: () => (
      <div className="relative">
        <Pushpin pinId="src-comms" tone="brass" className="-top-3 left-1/2 -translate-x-1/2" />
        <CommsDocument className="shadow-[0_30px_40px_-16px_rgba(0,0,0,.85)]" />
      </div>
    ),
  },
  {
    id: 'financial',
    index: '05',
    title: 'Financial & transactions',
    line: 'Payments placed on the same timeline as everything else.',
    tag: 'EV-04',
    meta: 'card_statement_mar2026.pdf · supplied',
    x: 10,
    y: 54,
    w: 15,
    rot: 2.5,
    render: () => (
      <div className="relative">
        <Pushpin pinId="src-financial" className="-top-3 left-1/2 -translate-x-1/2" />
        <TransactionSlip className="drop-shadow-[0_24px_24px_rgba(0,0,0,.8)]" />
      </div>
    ),
  },
  {
    id: 'location',
    index: '06',
    title: 'Location & network',
    line: 'Routes, access points, logins and IP records.',
    tag: 'EV-05',
    meta: 'warehouse_ap_assoc.log · route LOC-07',
    x: 37,
    y: 55,
    w: 27,
    rot: -1.5,
    render: () => (
      <EvidencePrint rot={0} pinId="src-location" tag="LOC-07" caption="Route reconstruction" imageClassName="aspect-[16/11]">
        <MapEvidence />
      </EvidencePrint>
    ),
  },
];

const LINKS: ThreadLink[] = [
  { id: 's1', from: 'src-cctv', to: 'src-mobile', delay: 0 },
  { id: 's2', from: 'src-mobile', to: 'src-fingerprint', delay: 150 },
  { id: 's3', from: 'src-fingerprint', to: 'src-comms', delay: 300 },
  // Lower row strung separately: a pin-to-pin link between rows would cross the captions.
  { id: 's5', from: 'src-financial', to: 'src-location', delay: 450 },
  { id: 's7', from: 'src-location', to: 'src-netlog', delay: 600 },
];

export const EvidenceSourcesBoard: React.FC = () => {
  const [ref, visible] = useInView<HTMLDivElement>({ rootMargin: '0px 0px -20% 0px' });
  return (
    <section id={SECTION_IDS.capabilities} aria-labelledby="sources-heading" className="board-surface relative overflow-hidden border-b border-ink-line py-section">
      <div aria-hidden="true" className="board-vignette" />
      <div className="relative mx-auto max-w-wide px-gutter">
        <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <EditorialHeading
            id="sources-heading"
            index="01"
            kicker="Evidence sources"
            title="Six kinds of evidence."
            emphasis="One board."
            lead="YUKTI ingests fragments from every source an investigation touches — and keeps each one traceable to its original."
          />
          <SyntheticStamp className="self-start lg:self-end">Synthetic demonstration artifacts</SyntheticStamp>
        </div>

        <div ref={ref} className="scene-3d relative mt-16 lg:mt-20 lg:h-[60rem] xl:h-[56rem]">
          <ThreadCanvas links={LINKS} drawn={visible} pulse={false} className="hidden lg:block" />
          <ul className="relative grid gap-x-8 gap-y-16 sm:grid-cols-2 lg:block lg:h-full">
            {SOURCES.map((source, i) => (
              <li
                key={source.id}
                className="settle group relative mx-auto w-full max-w-[22rem] sm:max-w-none lg:absolute lg:left-[var(--x)] lg:top-[var(--y)] lg:w-[var(--w)]"
                data-settled={visible}
                style={
                  {
                    '--x': `${source.x}%`,
                    '--y': `${source.y}%`,
                    '--w': `${source.w}%`,
                    '--settle-delay': `${i * 110}ms`,
                    '--settle-rot': `${source.rot - 6}deg`,
                  } as React.CSSProperties
                }
              >
                <figure className="is-liftable preserve-3d" aria-labelledby={`src-${source.id}-title`}>
                  <div
                    className={`mx-auto transition-transform duration-slow ease-out group-hover:-translate-y-2 ${source.id === 'mobile' ? 'w-1/2 sm:w-2/5 lg:w-full' : ''} ${
                      source.id === 'financial' ? 'w-3/4 lg:w-full' : ''
                    }`}
                    style={{ transform: `rotate(${source.rot}deg)` }}
                  >
                    {source.render()}
                  </div>
                  <figcaption className="mt-6 lg:mt-5">
                    <p className="flex items-baseline gap-3">
                      <span className="font-mono text-[0.6875rem] text-crimson">{source.index}</span>
                      <span id={`src-${source.id}-title`} className="font-serif text-xl text-fg-bright">
                        {source.title}
                      </span>
                    </p>
                    <p className="mt-1.5 max-w-[18rem] text-sm leading-snug text-fg-muted">{source.line}</p>
                    <p className="mt-2 font-mono text-[0.625rem] uppercase tracking-[0.1em] text-fg-subtle transition-colors duration-base group-hover:text-paper lg:opacity-0 lg:transition-opacity lg:group-hover:opacity-100 lg:group-focus-within:opacity-100">
                      <span className="text-crimson">{source.tag}</span> · {source.meta}
                    </p>
                  </figcaption>
                </figure>
              </li>
            ))}
            <li className="relative hidden lg:absolute lg:left-[71%] lg:top-[58%] lg:block lg:w-[19%]">
              <div style={{ transform: 'rotate(3deg)' }} className="relative">
                <Pushpin pinId="src-netlog" tone="steel" className="-top-3 left-1/2 -translate-x-1/2" />
                <LogPrintout title="AP association log" file="warehouse_ap_assoc.log" tag="EV-05" rows={NETWORK_LOG_ROWS} className="shadow-[0_30px_40px_-16px_rgba(0,0,0,.85)]" />
              </div>
              <p className="mt-6 max-w-[16rem] font-mono text-[0.625rem] uppercase leading-relaxed tracking-[0.12em] text-fg-subtle">
                Originals stay read-only. A SHA-256 digest is recorded the moment an artifact arrives.
              </p>
            </li>
          </ul>
        </div>
      </div>
    </section>
  );
};
