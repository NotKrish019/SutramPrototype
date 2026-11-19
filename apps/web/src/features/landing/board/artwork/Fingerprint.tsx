import React, { useId } from 'react';
import { FINGERPRINT_MINUTIAE, FINGERPRINT_RIDGES } from './fingerprintRidges';

export interface FingerprintProps {
  /** `scan`: light ridges on a dark scanner plate. `ink`: dark ink on card. `line`: hairline for decoration. */
  variant?: 'scan' | 'ink' | 'line';
  minutiae?: boolean;
  label?: string;
  className?: string;
}

/** Synthetic fingerprint artwork (procedurally generated — not a real print). */
export const Fingerprint: React.FC<FingerprintProps> = ({ variant = 'scan', minutiae = false, label, className = '' }) => {
  const id = useId().replace(/:/g, '');
  const ridge = variant === 'ink' ? '#2b2218' : variant === 'line' ? 'currentColor' : '#e6ded0';
  const width = variant === 'line' ? 0.9 : 1.5;
  return (
    <svg
      viewBox="0 0 200 240"
      className={`block h-full w-full ${className}`}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <defs>
        <radialGradient id={`plate-${id}`} cx="0.5" cy="0.45" r="0.7">
          <stop offset="0" stopColor="#24262a" />
          <stop offset="1" stopColor="#08090a" />
        </radialGradient>
        <filter id={`ink-${id}`}>
          <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed="9" result="n" />
          <feDisplacementMap in="SourceGraphic" in2="n" scale="0.9" />
        </filter>
        <linearGradient id={`sweep-${id}`} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" stopColor="#8FA9C4" stopOpacity="0" />
          <stop offset="0.5" stopColor="#c9dcf0" stopOpacity="0.5" />
          <stop offset="1" stopColor="#8FA9C4" stopOpacity="0" />
        </linearGradient>
      </defs>
      {variant === 'scan' && <rect width="200" height="240" fill={`url(#plate-${id})`} />}
      <g
        fill="none"
        stroke={ridge}
        strokeWidth={width}
        strokeLinecap="round"
        strokeLinejoin="round"
        opacity={variant === 'ink' ? 0.86 : variant === 'line' ? 0.5 : 0.82}
        filter={variant === 'line' ? undefined : `url(#ink-${id})`}
      >
        {FINGERPRINT_RIDGES.map((d, i) => (
          <path key={i} d={d} />
        ))}
      </g>
      {variant === 'scan' && (
        <>
          {/* scanner guides */}
          <g stroke="#8FA9C4" strokeOpacity="0.35" strokeWidth="0.6" fill="none">
            <path d="M12 12 h14 M12 12 v14 M188 12 h-14 M188 12 v14 M12 228 h14 M12 228 v-14 M188 228 h-14 M188 228 v-14" strokeOpacity="0.8" />
            <line x1="100" y1="6" x2="100" y2="234" strokeDasharray="2 4" />
            <line x1="6" y1="120" x2="194" y2="120" strokeDasharray="2 4" />
          </g>
          <rect x="0" y="0" width="200" height="34" fill={`url(#sweep-${id})`} className="scan-sweep" />
        </>
      )}
      {minutiae &&
        FINGERPRINT_MINUTIAE.map((m) => (
          <g key={m.id}>
            <circle cx={m.x} cy={m.y} r="6" fill="none" stroke="#E32636" strokeWidth="1.2" />
            <line x1={m.x + 6} y1={m.y - 3} x2={m.x + 18} y2={m.y - 9} stroke="#E32636" strokeWidth="0.8" />
            <text x={m.x + 19} y={m.y - 10} fontFamily="JetBrains Mono, monospace" fontSize="7" fill="#E32636">
              {m.id}
            </text>
          </g>
        ))}
    </svg>
  );
};
