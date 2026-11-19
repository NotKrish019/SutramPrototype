import React, { useId } from 'react';

/* ------------------------------------------------------------------ Pushpin */

export type PinTone = 'crimson' | 'steel' | 'brass';

const PIN_TONES: Record<PinTone, [string, string, string]> = {
  crimson: ['#ff6a74', '#d0202f', '#5e0b12'],
  steel: ['#f2f4f6', '#9aa5b1', '#3c444d'],
  brass: ['#f6e3b0', '#c9a45a', '#5d4517'],
};

/**
 * A metal-shanked pushpin seen from slightly above: glossy head, collar and a
 * short visible needle casting a shadow on the surface below.
 */
export const Pushpin: React.FC<{ tone?: PinTone; className?: string; style?: React.CSSProperties; pinId?: string }> = ({
  tone = 'crimson',
  className = '',
  style,
  pinId,
}) => {
  const id = useId().replace(/:/g, '');
  const [hi, mid, lo] = PIN_TONES[tone];
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 28 34"
      data-pin-id={pinId}
      className={`pushpin pointer-events-none absolute z-20 h-[30px] w-[25px] ${className}`}
      style={style}
    >
      <defs>
        <radialGradient id={`h-${id}`} cx="38%" cy="30%" r="70%">
          <stop offset="0" stopColor={hi} />
          <stop offset="0.45" stopColor={mid} />
          <stop offset="1" stopColor={lo} />
        </radialGradient>
        <linearGradient id={`n-${id}`} x1="0" x2="1">
          <stop offset="0" stopColor="#5a5a5a" />
          <stop offset="0.5" stopColor="#e8e8e8" />
          <stop offset="1" stopColor="#6a6a6a" />
        </linearGradient>
      </defs>
      {/* needle */}
      <path d="M13.2 21 L14 33 L14.8 21 Z" fill={`url(#n-${id})`} />
      {/* collar */}
      <ellipse cx="14" cy="20" rx="6.2" ry="2.4" fill={lo} />
      <path d="M8.6 14 Q 14 23 19.4 14 L 19.8 19 Q 14 23.5 8.2 19 Z" fill={mid} />
      {/* head */}
      <circle cx="14" cy="11" r="9" fill={`url(#h-${id})`} />
      <ellipse cx="10.6" cy="7" rx="3.2" ry="2" fill="#fff" opacity="0.55" />
    </svg>
  );
};

/* ------------------------------------------------------------------- Thread */

export type ThreadState = 'idle' | 'hot' | 'dim';

/** Quadratic path between two points that sags under its own weight. */
export function sag(x1: number, y1: number, x2: number, y2: number, amount = 0.12): string {
  const mx = (x1 + x2) / 2;
  const my = (y1 + y2) / 2 + Math.hypot(x2 - x1, y2 - y1) * amount;
  return `M ${x1} ${y1} Q ${mx} ${my} ${x2} ${y2}`;
}

export interface ThreadProps {
  d: string;
  /** Observed links are drawn solid; inferred links dashed — never conflated. */
  kind?: 'observed' | 'inferred';
  state?: ThreadState;
  delay?: number;
  pulse?: boolean;
  shadow?: boolean;
}

/** Red investigation string with a cast shadow. Draws in when an ancestor has `data-drawn="true"`. */
export const Thread: React.FC<ThreadProps> = ({ d, kind = 'observed', state = 'idle', delay = 0, pulse = false, shadow = true }) => {
  const style = { '--draw-delay': `${delay}ms` } as React.CSSProperties;
  return (
    <g>
      {shadow && <path d={d} className="thread-shadow" opacity={state === 'dim' ? 0.15 : 0.6} />}
      <path
        d={d}
        pathLength={kind === 'observed' ? 1 : undefined}
        className={`thread ${kind === 'observed' ? 'thread--draw' : 'thread--inferred'}`}
        data-state={state}
        style={style}
      />
      {pulse && state === 'hot' && kind === 'observed' && <path d={d} pathLength={1} className="thread-pulse" />}
    </g>
  );
};

/* ----------------------------------------------------------- Evidence print */

export interface EvidencePrintProps {
  /** Resting rotation in degrees. */
  rot?: number;
  /** Resting depth in px (positive = closer to the viewer). */
  z?: number;
  pin?: PinTone | null;
  /** Anchor id for threads drawn by ThreadCanvas. */
  pinId?: string;
  pinPosition?: 'center' | 'left' | 'right';
  dark?: boolean;
  /** Evidence identifier printed on the border, e.g. EV-001. */
  tag?: string;
  /** Short caption written on the lower border. */
  caption?: React.ReactNode;
  active?: boolean;
  className?: string;
  imageClassName?: string;
  style?: React.CSSProperties;
  children: React.ReactNode;
}

/**
 * A paper-bordered photographic print pinned to the board. Lifts towards the
 * viewer on hover/focus (or when `active`), with its pin catching the light.
 */
export const EvidencePrint: React.FC<EvidencePrintProps> = ({
  rot = 0,
  z = 0,
  pin = 'crimson',
  pinId,
  pinPosition = 'center',
  dark = false,
  tag,
  caption,
  active = false,
  className = '',
  imageClassName = '',
  style,
  children,
}) => (
  <div className={`is-liftable preserve-3d ${className}`} data-active={active} style={style}>
    <div
      className={`evidence-print ${dark ? 'evidence-print--dark' : ''}`}
      style={{ '--rot': `${rot}deg`, '--z': `${z}px` } as React.CSSProperties}
    >
      {pin && (
        <Pushpin
          tone={pin}
          pinId={pinId}
          className={
            pinPosition === 'center'
              ? 'left-1/2 -top-4 -translate-x-1/2'
              : pinPosition === 'left'
                ? 'left-5 -top-4'
                : 'right-5 -top-4'
          }
        />
      )}
      <div className={`evidence-print__image ${imageClassName}`}>{children}</div>
      {(tag || caption) && (
        <div
          className={`mt-1.5 flex items-baseline justify-between gap-3 font-mono text-[0.625rem] uppercase leading-tight tracking-[0.08em] ${
            dark ? 'text-fg-muted' : 'text-[#4a3b28]'
          }`}
        >
          {caption && <span className="min-w-0 truncate">{caption}</span>}
          {tag && <span className={dark ? 'text-crimson' : 'text-crimson-deep'}>{tag}</span>}
        </div>
      )}
    </div>
  </div>
);

/* --------------------------------------------------------------- Annotation */

/** A small mono label set on the board, e.g. `CCTV-03 · observed`. */
export const BoardLabel: React.FC<{ children: React.ReactNode; tone?: 'muted' | 'crimson' | 'paper'; className?: string }> = ({
  children,
  tone = 'muted',
  className = '',
}) => (
  <span
    className={`inline-flex items-center gap-1.5 font-mono text-[0.625rem] uppercase tracking-[0.12em] ${
      tone === 'crimson' ? 'text-crimson' : tone === 'paper' ? 'text-paper' : 'text-fg-subtle'
    } ${className}`}
  >
    {children}
  </span>
);

/** Ribbon stating that a visual uses synthetic demonstration data. */
export const SyntheticStamp: React.FC<{ className?: string; children?: React.ReactNode }> = ({ className = '', children }) => (
  <span
    className={`inline-flex items-center gap-2 border border-crimson/50 px-2 py-1 font-mono text-[0.625rem] uppercase tracking-[0.14em] text-crimson ${className}`}
  >
    <span aria-hidden="true" className="h-1 w-1 rounded-full bg-crimson" />
    {children ?? 'Synthetic data'}
  </span>
);

/* -------------------------------------------------------- Section heading */

export interface EditorialHeadingProps {
  id: string;
  index: string;
  kicker: string;
  /** Main line, set roman. */
  title: React.ReactNode;
  /** Emphasis line, set italic in crimson. */
  emphasis?: React.ReactNode;
  lead?: React.ReactNode;
  align?: 'left' | 'center';
  className?: string;
}

/** Editorial section heading: mono case index, serif headline with an italic turn, one-line lead. */
export const EditorialHeading: React.FC<EditorialHeadingProps> = ({
  id,
  index,
  kicker,
  title,
  emphasis,
  lead,
  align = 'left',
  className = '',
}) => (
  <div className={`${align === 'center' ? 'mx-auto text-center' : ''} max-w-3xl ${className}`}>
    <p
      className={`mb-6 flex items-center gap-3 font-mono text-[0.6875rem] uppercase tracking-[0.18em] text-fg-subtle ${
        align === 'center' ? 'justify-center' : ''
      }`}
    >
      <span className="text-crimson">{index}</span>
      <span aria-hidden="true" className="h-px w-8 bg-crimson/70" />
      {kicker}
    </p>
    <h2 id={id} className="font-serif text-editorial-lg font-normal text-fg-bright">
      {title}
      {emphasis && (
        <>
          {' '}
          <em className="font-light italic text-crimson">{emphasis}</em>
        </>
      )}
    </h2>
    {lead && <p className={`mt-6 max-w-xl text-base leading-relaxed text-fg-muted sm:text-lg ${align === 'center' ? 'mx-auto' : ''}`}>{lead}</p>}
  </div>
);
