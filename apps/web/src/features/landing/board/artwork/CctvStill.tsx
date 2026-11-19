import React, { useId } from 'react';

export interface CctvStillProps {
  camera?: string;
  location?: string;
  timestamp?: string;
  /** Draws a dashed box around the vehicle, as an analyst annotation. */
  annotate?: boolean;
  className?: string;
}

/**
 * Original, fully synthetic CCTV still: a night-time loading gate seen from a
 * high wall camera, rendered in monochrome with sensor noise and an on-screen
 * display. No photographic source material.
 */
export const CctvStill: React.FC<CctvStillProps> = ({
  camera = 'CAM 03',
  location = 'GATE B',
  timestamp = '2026-03-14 22:41:07',
  annotate = false,
  className = '',
}) => {
  const id = useId().replace(/:/g, '');
  return (
    <svg viewBox="0 0 320 220" className={`block h-full w-full ${className}`} role="img" aria-label={`Synthetic CCTV still, ${camera} ${location}, ${timestamp}`}>
      <defs>
        <linearGradient id={`sky-${id}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#0d0e0e" />
          <stop offset="1" stopColor="#202221" />
        </linearGradient>
        <linearGradient id={`ground-${id}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#2c2d2b" />
          <stop offset="1" stopColor="#141514" />
        </linearGradient>
        <radialGradient id={`lamp-${id}`} cx="0.5" cy="0" r="0.9">
          <stop offset="0" stopColor="#d9d9d4" stopOpacity="0.85" />
          <stop offset="0.35" stopColor="#8d8d88" stopOpacity="0.35" />
          <stop offset="1" stopColor="#000" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={`shutter-${id}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#6c6d69" />
          <stop offset="1" stopColor="#2b2c2a" />
        </linearGradient>
        <filter id={`noise-${id}`} x="0" y="0" width="100%" height="100%">
          <feTurbulence type="fractalNoise" baseFrequency="1.1" numOctaves="2" seed="4" result="t" />
          <feColorMatrix in="t" values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 0.22 0" />
        </filter>
        <radialGradient id={`vig-${id}`} cx="0.5" cy="0.5" r="0.75">
          <stop offset="0.55" stopColor="#000" stopOpacity="0" />
          <stop offset="1" stopColor="#000" stopOpacity="0.75" />
        </radialGradient>
        <pattern id={`scan-${id}`} width="4" height="3" patternUnits="userSpaceOnUse">
          <rect width="4" height="1" fill="#fff" opacity="0.05" />
        </pattern>
      </defs>

      <rect width="320" height="220" fill={`url(#sky-${id})`} />
      {/* ground plane with perspective lane markings */}
      <path d="M0 132 L320 120 L320 220 L0 220 Z" fill={`url(#ground-${id})`} />
      {[0, 1, 2, 3, 4].map((i) => (
        <path key={i} d={`M${150 + i * 6} ${128 - i} L${40 + i * 70} 220`} stroke="#4a4b48" strokeWidth="0.7" opacity="0.5" />
      ))}
      <path d="M118 150 L300 140" stroke="#5b5c58" strokeDasharray="10 8" strokeWidth="1.2" opacity="0.6" />

      {/* warehouse facade in two-point perspective */}
      <path d="M150 30 L320 18 L320 124 L150 132 Z" fill="#242523" />
      <path d="M0 40 L150 30 L150 132 L0 136 Z" fill="#1b1c1b" />
      {Array.from({ length: 9 }).map((_, i) => (
        <path key={i} d={`M${150 + i * 19} ${30 - i * 1.35} L${150 + i * 19} ${132 - i * 0.9}`} stroke="#2f302d" strokeWidth="0.8" />
      ))}
      {Array.from({ length: 7 }).map((_, i) => (
        <path key={i} d={`M${i * 22} ${40 - i * 1.4} L${i * 22} ${136 - i * 0.6}`} stroke="#252624" strokeWidth="0.8" />
      ))}
      {/* roller shutter gate (Gate B), partially raised */}
      <path d="M196 60 L268 55 L268 126 L196 129 Z" fill="#0e0f0e" />
      <path d="M196 60 L268 55 L268 96 L196 100 Z" fill={`url(#shutter-${id})`} />
      {Array.from({ length: 10 }).map((_, i) => (
        <path key={i} d={`M196 ${64 + i * 3.6} L268 ${59 + i * 3.6}`} stroke="#1f201e" strokeWidth="0.9" />
      ))}
      <text x="222" y="52" fontFamily="JetBrains Mono, monospace" fontSize="7" fill="#9b9c97" opacity="0.8">B</text>
      {/* wall lamp and its light pool */}
      <rect x="226" y="40" width="10" height="3" fill="#e9e9e3" />
      <ellipse cx="231" cy="120" rx="95" ry="60" fill={`url(#lamp-${id})`} opacity="0.55" />
      <path d="M231 43 L170 132 L292 126 Z" fill="#f1f1ea" opacity="0.05" />

      {/* vehicle, rear three-quarter view */}
      <g transform="translate(132 112)">
        <path d="M6 30 Q 4 14 22 10 L 40 2 Q 58 -2 76 4 L 92 14 Q 100 18 100 30 Z" fill="#3c3d3a" />
        <path d="M26 11 L 42 4 Q 56 1 72 6 L 82 14 Z" fill="#141514" />
        <rect x="4" y="28" width="98" height="10" rx="2" fill="#2a2b29" />
        <rect x="8" y="22" width="9" height="4" fill="#b5b5ad" opacity="0.85" />
        <rect x="88" y="21" width="9" height="4" fill="#b5b5ad" opacity="0.85" />
        <rect x="40" y="30" width="22" height="6" fill="#8d8e88" />
        <text x="42.5" y="35" fontFamily="JetBrains Mono, monospace" fontSize="4.6" fill="#1a1a19">
          ·· ·47
        </text>
        <ellipse cx="22" cy="40" rx="8" ry="4" fill="#0a0a0a" />
        <ellipse cx="84" cy="40" rx="8" ry="4" fill="#0a0a0a" />
        <ellipse cx="52" cy="44" rx="56" ry="5" fill="#000" opacity="0.45" />
      </g>
      {annotate && (
        <g>
          <rect x="128" y="104" width="112" height="58" fill="none" stroke="#E32636" strokeWidth="1.2" strokeDasharray="4 3" />
          <rect x="128" y="96" width="64" height="9" fill="#E32636" />
          <text x="131" y="102.6" fontFamily="JetBrains Mono, monospace" fontSize="6" fill="#fff">
            VEH-1 · plate …47
          </text>
        </g>
      )}

      {/* sensor noise, scanlines, vignette */}
      <rect width="320" height="220" filter={`url(#noise-${id})`} />
      <rect width="320" height="220" fill={`url(#scan-${id})`} />
      <rect width="320" height="220" fill={`url(#vig-${id})`} />

      {/* on-screen display */}
      <g fontFamily="JetBrains Mono, monospace" fontSize="8.5" fill="#e8e8e2">
        <text x="10" y="16">
          {camera} · {location}
        </text>
        <text x="310" y="16" textAnchor="end">
          {timestamp}
        </text>
        <circle cx="13" cy="205" r="3" fill="#E32636" className="rec-blink" />
        <text x="21" y="208" fontSize="7.5">
          REC
        </text>
        <text x="310" y="208" textAnchor="end" fontSize="7" opacity="0.7">
          SYNTHETIC
        </text>
      </g>
    </svg>
  );
};
