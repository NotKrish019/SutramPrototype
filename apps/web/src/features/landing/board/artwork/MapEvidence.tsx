import React, { useId } from 'react';
import { seeded } from '../hooks';

/** Fictional street network (deterministic) — not a real place. */
function buildStreets(): { major: string[]; minor: string[] } {
  const rand = seeded(714);
  const major = [
    'M -10 60 C 60 50, 110 90, 170 80 S 260 40, 330 58',
    'M 40 -10 C 60 60, 50 120, 92 170 S 120 220, 110 240',
    'M 180 -10 C 170 50, 200 110, 190 160 S 230 210, 250 240',
    'M -10 150 C 70 150, 120 130, 200 150 S 290 180, 330 170',
  ];
  const minor: string[] = [];
  for (let i = 0; i < 26; i++) {
    const x = rand() * 320;
    const y = rand() * 220;
    const a = rand() * Math.PI;
    const l = 25 + rand() * 60;
    const bend = (rand() - 0.5) * 30;
    const x2 = x + Math.cos(a) * l;
    const y2 = y + Math.sin(a) * l;
    minor.push(`M ${x.toFixed(1)} ${y.toFixed(1)} Q ${((x + x2) / 2 + bend).toFixed(1)} ${((y + y2) / 2 - bend).toFixed(1)} ${x2.toFixed(1)} ${y2.toFixed(1)}`);
  }
  return { major, minor };
}

const STREETS = buildStreets();

/** Route points: fuel station → approach → Warehouse 7 Gate B. */
const ROUTE = 'M 58 196 C 80 170, 88 160, 96 150 S 140 132, 168 112 S 214 86, 236 70';

export const MapEvidence: React.FC<{ className?: string; label?: boolean }> = ({ className = '', label = true }) => {
  const id = useId().replace(/:/g, '');
  return (
    <svg viewBox="0 0 320 220" className={`block h-full w-full ${className}`} role="img" aria-label="Synthetic location map: route from a fuel station to Warehouse 7 Gate B">
      <defs>
        <radialGradient id={`m-${id}`} cx="0.6" cy="0.4" r="0.8">
          <stop offset="0" stopColor="#1d1e1f" />
          <stop offset="1" stopColor="#0b0b0c" />
        </radialGradient>
        <filter id={`g-${id}`} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="2.4" />
        </filter>
      </defs>
      <rect width="320" height="220" fill={`url(#m-${id})`} />
      {/* blocks */}
      <g fill="#151617">
        <path d="M110 20 L160 16 L166 60 L116 66 Z" />
        <path d="M220 96 L290 90 L296 140 L226 146 Z" />
        <path d="M18 80 L70 76 L74 128 L22 132 Z" />
      </g>
      <g fill="none" strokeLinecap="round">
        {STREETS.minor.map((d, i) => (
          <path key={i} d={d} stroke="#2b2c2d" strokeWidth="1.2" />
        ))}
        {STREETS.major.map((d, i) => (
          <path key={i} d={d} stroke="#3d3e3f" strokeWidth="3.4" />
        ))}
        {STREETS.major.map((d, i) => (
          <path key={`c-${i}`} d={d} stroke="#151617" strokeWidth="0.6" strokeDasharray="4 5" />
        ))}
      </g>
      {/* route */}
      <path d={ROUTE} fill="none" stroke="#E5243B" strokeWidth="5" opacity="0.4" filter={`url(#g-${id})`} />
      <path d={ROUTE} fill="none" stroke="#ff4150" strokeWidth="1.8" strokeDasharray="5 4" strokeLinecap="round" />
      {[
        [58, 196],
        [96, 150],
        [168, 112],
      ].map(([x, y]) => (
        <circle key={`${x}`} cx={x} cy={y} r="3.2" fill="#E5243B" stroke="#ffd6da" strokeWidth="0.8" />
      ))}
      {/* destination pin */}
      <g transform="translate(236 70)">
        <ellipse cx="0" cy="1" rx="6" ry="2" fill="#000" opacity="0.6" />
        <path d="M0 0 C -9 -12, -9 -22, 0 -24 C 9 -22, 9 -12, 0 0 Z" fill="#E5243B" />
        <circle cx="0" cy="-16" r="3.2" fill="#2a0a0d" />
      </g>
      {label && (
        <g fontFamily="JetBrains Mono, monospace" fontSize="7.5" fill="#bdb6aa">
          <text x="244" y="58">WH-7 · GATE B</text>
          <text x="66" y="208">FUEL · 21:58</text>
          <text x="10" y="16" fill="#71717A">LOC-07 · SYNTHETIC</text>
        </g>
      )}
    </svg>
  );
};
