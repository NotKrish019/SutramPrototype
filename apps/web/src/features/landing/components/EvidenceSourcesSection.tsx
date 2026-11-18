import React, { useState } from 'react';
import { EVIDENCE_SOURCES, SECTION_IDS, type SourceId } from '../data/siteContent';
import { EvidenceNetworkVisual } from './EvidenceNetworkVisual';
import { Container, Eyebrow, Frame, Reveal } from '../../../components/ui/primitives';

const MONO = 'JetBrains Mono, ui-monospace, monospace';
const HOVER_AMBER = 'transition-colors duration-base group-hover:fill-signal-amber group-focus-within:fill-signal-amber';

const MobileGlyph: React.FC = () => (
  <svg viewBox="0 0 220 120" className="h-full w-full" aria-hidden="true">
    <rect x="16" y="6" width="62" height="108" rx="7" fill="none" stroke="#36363C" />
    <rect x="38" y="11" width="18" height="3" fill="#36363C" />
    <rect x="24" y="26" width="36" height="9" fill="#232327" />
    <rect x="34" y="40" width="36" height="9" className={`fill-[#EDEDEF] ${HOVER_AMBER}`} />
    <rect x="24" y="54" width="28" height="9" fill="#232327" />
    <rect x="30" y="68" width="40" height="9" fill="#232327" />
    <line x1="70" y1="44" x2="112" y2="44" stroke="#36363C" strokeDasharray="2 3" />
    <g fontFamily={MONO} fontSize="9" fill="#A3A3AB">
      <text x="118" y="34" fill="#6E6E77">MESSAGE</text>
      <text x="118" y="48">"at the gate in 5"</text>
      <text x="118" y="66" fill="#6E6E77">SENT</text>
      <text x="118" y="80">22:36:14 · D-01</text>
      <text x="118" y="98" fill="#6E6E77">SOURCE</text>
      <text x="118" y="112">chat_export.json</text>
    </g>
  </svg>
);

const CctvGlyph: React.FC = () => (
  <svg viewBox="0 0 220 120" className="h-full w-full" aria-hidden="true">
    <defs>
      <clipPath id="cctv-frame">
        <rect x="10" y="8" width="200" height="104" />
      </clipPath>
    </defs>
    <rect x="10" y="8" width="200" height="104" fill="#0F0F11" stroke="#36363C" />
    <g clipPath="url(#cctv-frame)" stroke="#232327">
      {Array.from({ length: 9 }, (_, i) => (
        <line key={`v${i}`} x1={10 + i * 25} y1="8" x2={10 + i * 25} y2="112" />
      ))}
      {Array.from({ length: 5 }, (_, i) => (
        <line key={`h${i}`} x1="10" y1={8 + i * 26} x2="210" y2={8 + i * 26} />
      ))}
      <rect x="92" y="58" width="54" height="30" fill="none" stroke="#EDEDEF" strokeDasharray="3 3" className="transition-colors duration-base group-hover:stroke-signal-amber" />
      <line
        x1="10"
        x2="210"
        y1="10"
        y2="10"
        stroke="#FFB224"
        strokeOpacity="0.6"
        className="scan-line [animation-play-state:paused] group-hover:[animation-play-state:running]"
        style={{ '--scan-distance': '100px' } as React.CSSProperties}
      />
    </g>
    <g fontFamily={MONO} fontSize="8.5">
      <text x="16" y="22" fill="#EDEDEF">CAM 03 · GATE B</text>
      <text x="204" y="22" fill="#FF5A4E" textAnchor="end">● REC</text>
      <text x="16" y="104" fill="#A3A3AB">2026-03-14 22:41:07</text>
      <text x="204" y="104" fill="#6E6E77" textAnchor="end">CAMERA CLOCK</text>
    </g>
  </svg>
);

const EmailGlyph: React.FC = () => (
  <svg viewBox="0 0 220 120" className="h-full w-full" aria-hidden="true">
    <rect x="10" y="8" width="200" height="104" fill="#0F0F11" stroke="#36363C" />
    <g fontFamily={MONO} fontSize="8.5">
      {[
        ['FROM', 'ops@example.test'],
        ['TO', 'gate-desk@example.test'],
        ['DATE', 'Sat, 14 Mar 2026 21:12'],
        ['SUBJ', 'Delivery window changed'],
      ].map(([key, value], i) => (
        <g key={key}>
          <text x="20" y={28 + i * 16} fill="#6E6E77">{key}</text>
          <text x="58" y={28 + i * 16} fill="#A3A3AB">{value}</text>
        </g>
      ))}
    </g>
    <line x1="20" x2="200" y1="92" y2="92" stroke="#232327" />
    <rect x="20" y="98" width="70" height="6" className={`fill-[#36363C] ${HOVER_AMBER}`} />
    <rect x="96" y="98" width="40" height="6" fill="#232327" />
  </svg>
);

const SocialGlyph: React.FC = () => {
  const nodes = [
    [110, 60],
    [52, 30],
    [56, 94],
    [168, 28],
    [172, 92],
    [110, 16],
  ] as const;
  return (
    <svg viewBox="0 0 220 120" className="h-full w-full" aria-hidden="true">
      {nodes.slice(1).map(([x, y]) => (
        <line key={`${x}-${y}`} x1="110" y1="60" x2={x} y2={y} stroke="#36363C" />
      ))}
      <line x1="52" y1="30" x2="110" y2="16" stroke="#36363C" strokeDasharray="2 3" />
      {nodes.map(([x, y], i) => (
        <circle
          key={`${x}-${y}`}
          cx={x}
          cy={y}
          r={i === 0 ? 9 : 5}
          fill={i === 0 ? '#09090A' : '#232327'}
          stroke={i === 0 ? '#EDEDEF' : '#6E6E77'}
          className={i === 3 ? 'transition-colors duration-base group-hover:fill-signal-amber group-hover:stroke-signal-amber' : undefined}
        />
      ))}
      <text x="110" y="114" textAnchor="middle" fontFamily={MONO} fontSize="8.5" fill="#6E6E77">
        ACCOUNT · INTERACTIONS
      </text>
    </svg>
  );
};

const FinancialGlyph: React.FC = () => (
  <svg viewBox="0 0 220 120" className="h-full w-full" aria-hidden="true">
    <g fontFamily={MONO} fontSize="8.5">
      <text x="12" y="16" fill="#6E6E77">TIME</text>
      <text x="64" y="16" fill="#6E6E77">MERCHANT</text>
      <text x="208" y="16" fill="#6E6E77" textAnchor="end">AMOUNT</text>
      {[
        ['19:04', 'Grocery', '1,240.00'],
        ['21:58', 'Fuel station', '2,000.00'],
        ['22:51', 'Toll plaza', '85.00'],
        ['23:30', 'ATM withdrawal', '5,000.00'],
      ].map(([time, merchant, amount], i) => (
        <g key={time}>
          <rect
            x="6"
            y={26 + i * 22}
            width="208"
            height="18"
            className={i === 1 ? 'fill-transparent transition-colors duration-base group-hover:fill-signal-amber/15' : 'fill-transparent'}
          />
          <line x1="6" x2="214" y1={26 + i * 22} y2={26 + i * 22} stroke="#232327" />
          <text x="12" y={38 + i * 22} fill={i === 1 ? '#EDEDEF' : '#A3A3AB'}>{time}</text>
          <text x="64" y={38 + i * 22} fill={i === 1 ? '#EDEDEF' : '#A3A3AB'}>{merchant}</text>
          <text x="208" y={38 + i * 22} fill={i === 1 ? '#EDEDEF' : '#A3A3AB'} textAnchor="end">{amount}</text>
        </g>
      ))}
    </g>
  </svg>
);

const NetworkGlyph: React.FC = () => (
  <svg viewBox="0 0 520 120" className="h-full w-full" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
    <g fontFamily={MONO} fontSize="8.5">
      {['AP-WH7-02', 'VPN-GW', 'CLOUD-STORE'].map((label, row) => (
        <g key={label}>
          <text x="0" y={28 + row * 36} fill="#6E6E77">{label}</text>
          <line x1="88" x2="516" y1={25 + row * 36} y2={25 + row * 36} stroke="#232327" />
          {Array.from({ length: 14 }, (_, i) => {
            const x = 92 + i * 30 + ((row * 7 + i * 3) % 11);
            const hit = row === 0 && i === 9;
            return (
              <rect
                key={i}
                x={x}
                y={19 + row * 36}
                width={hit ? 10 : 4 + ((i + row) % 4) * 3}
                height="12"
                fill={hit ? '#FFB224' : '#36363C'}
                className={hit ? undefined : 'transition-colors duration-base group-hover:fill-[#4a4a52]'}
              />
            );
          })}
        </g>
      ))}
      <text x="372" y="12" fill="#FFB224">22:44:10 · D-01 ASSOC</text>
    </g>
  </svg>
);

const GLYPHS: Record<SourceId, React.FC> = {
  mobile: MobileGlyph,
  cctv: CctvGlyph,
  email: EmailGlyph,
  social: SocialGlyph,
  financial: FinancialGlyph,
  network: NetworkGlyph,
};


export const EvidenceSourcesSection: React.FC = () => {
  const [active, setActive] = useState<SourceId | null>(null);
  const previewId: SourceId = active ?? 'cctv';
  const preview = EVIDENCE_SOURCES.find((source) => source.id === previewId)!;
  const Glyph = GLYPHS[previewId];

  return (
    <section id={SECTION_IDS.sources} aria-labelledby="sources-heading" className="border-b border-ink-line py-section">
      <Container>
        <Reveal className="grid gap-10 lg:grid-cols-12 lg:items-center">
          <div className="lg:col-span-5">
            <Eyebrow className="mb-5">01 — Evidence sources</Eyebrow>
            <h2 id="sources-heading" className="font-display text-[clamp(2rem,3.6vw,3.25rem)] uppercase leading-[0.95] tracking-[-0.015em] text-fg">
              <span className="block font-light">Every source matters.</span>{' '}
              <span className="block font-bold">Every connection counts.</span>
            </h2>
            <p className="mt-5 max-w-md text-base text-fg-muted">Six kinds of evidence, one case model.</p>

            <ul className="mt-8 divide-y divide-ink-line border-y border-ink-line" aria-label="Evidence source types">
              {EVIDENCE_SOURCES.map((source) => {
                const isActive = active === source.id;
                return (
                  <li key={source.id}>
                    <button
                      type="button"
                      aria-pressed={isActive}
                      onMouseEnter={() => setActive(source.id)}
                      onFocus={() => setActive(source.id)}
                      onClick={() => setActive((current) => (current === source.id ? null : source.id))}
                      className={`group grid w-full grid-cols-[2rem_1fr] items-baseline gap-x-3 py-3 text-left transition-colors focus:outline-none focus-visible:bg-white/[0.03] ${
                        isActive ? 'text-fg' : 'text-fg-muted hover:text-fg'
                      }`}
                    >
                      <span className={`font-mono text-micro ${isActive ? 'text-signal-amber' : 'text-fg-subtle'}`}>{source.index}</span>
                      <span>
                        <span className="block font-display text-lg font-semibold uppercase leading-tight">{source.title}</span>
                        <span className="block text-sm text-fg-subtle">{source.description}</span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>

          <div className="flex min-w-0 flex-col gap-4 lg:col-span-7" onMouseLeave={() => setActive(null)}>
            <EvidenceNetworkVisual active={active} onActiveChange={setActive} />
            <Frame title={<span>Artifact preview · {preview.title}</span>} meta="Synthetic" bodyClassName="h-32 px-4 py-3 sm:h-36">
              <div key={previewId} className="h-full animate-[fadeIn_400ms_cubic-bezier(0.22,1,0.36,1)]">
                <Glyph />
              </div>
            </Frame>
          </div>
        </Reveal>
      </Container>
    </section>
  );
};
