import React from 'react';
import { ArrowRight } from 'lucide-react';
import { Link } from '../../router/Link';
import { useInView } from '../../features/landing/hooks/useInView';
import {
  PROVENANCE_DESCRIPTION,
  PROVENANCE_LABEL,
  REVIEW_LABEL,
  type ProvenanceClass,
  type ReviewStatus,
} from '../../fixtures/synthetic/case026';

/* ------------------------------------------------------------------ Layout */

export const Container: React.FC<{ className?: string; children: React.ReactNode }> = ({
  className = '',
  children,
}) => <div className={`mx-auto w-full max-w-site px-gutter ${className}`}>{children}</div>;

/** Fades content in once when it enters the viewport. Never hides content without JS support. */
export const Reveal: React.FC<{
  delay?: number;
  className?: string;
  children: React.ReactNode;
}> = ({ delay = 0, className = '', children }) => {
  const [ref, visible] = useInView<HTMLDivElement>();
  return (
    <div
      ref={ref}
      data-visible={visible}
      className={`reveal ${className}`}
      style={{ '--reveal-delay': `${delay}ms` } as React.CSSProperties}
    >
      {children}
    </div>
  );
};

/* -------------------------------------------------------------- Typography */

export const Eyebrow: React.FC<{ children: React.ReactNode; className?: string }> = ({
  children,
  className = '',
}) => (
  <p className={`font-mono text-eyebrow uppercase text-fg-subtle ${className}`}>
    <span aria-hidden="true">[ </span>
    {children}
    <span aria-hidden="true"> ]</span>
  </p>
);

export interface SectionHeaderProps {
  id?: string;
  eyebrow: string;
  /** First headline line, set light. */
  lead: string;
  /** Second headline line, set bold. */
  emphasis: string;
  aside?: React.ReactNode;
  headingLevel?: 'h2' | 'h3';
}

/** Two-weight uppercase headline on the left, supporting copy on the right. */
export const SectionHeader: React.FC<SectionHeaderProps> = ({
  id,
  eyebrow,
  lead,
  emphasis,
  aside,
  headingLevel: Heading = 'h2',
}) => (
  <Reveal className="grid gap-8 lg:grid-cols-12 lg:items-end">
    <div className="lg:col-span-8">
      <Eyebrow className="mb-5">{eyebrow}</Eyebrow>
      <Heading id={id} className="font-display text-display-lg uppercase text-fg">
        <span className="block font-light">{lead}</span>{' '}
        <span className="block font-bold">{emphasis}</span>
      </Heading>
    </div>
    {aside && (
      <div className="text-base leading-relaxed text-fg-muted lg:col-span-4 lg:pb-2">{aside}</div>
    )}
  </Reveal>
);

/* ----------------------------------------------------------------- Actions */

type CtaVariant = 'primary' | 'secondary' | 'ghost';

const CTA_STYLES: Record<CtaVariant, string> = {
  primary:
    'bg-crimson text-fg-bright border border-crimson shadow-[0_10px_30px_-10px_rgba(227,38,54,0.7)] hover:bg-[#f0303f] hover:shadow-[0_14px_40px_-10px_rgba(227,38,54,0.85)] active:bg-crimson-deep',
  secondary:
    'border border-paper/30 text-fg hover:border-paper/70 hover:bg-paper/[0.06] active:bg-paper/[0.1]',
  ghost: 'text-fg-muted hover:text-fg',
};

const CTA_BASE =
  'group inline-flex items-center justify-center gap-2 px-5 py-3 font-mono text-[0.75rem] uppercase tracking-[0.12em] transition-[color,background-color,border-color,box-shadow] duration-base ease-out focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson focus-visible:ring-offset-2 focus-visible:ring-offset-ink-950';

export interface CtaLinkProps {
  href: string;
  variant?: CtaVariant;
  arrow?: boolean;
  className?: string;
  children: React.ReactNode;
  onClick?: () => void;
}

/** CTA styled anchor. In-page hashes use native anchors; app routes use the client router. */
export const CtaLink: React.FC<CtaLinkProps> = ({
  href,
  variant = 'primary',
  arrow = false,
  className = '',
  children,
  onClick,
}) => {
  const classes = `${CTA_BASE} ${CTA_STYLES[variant]} ${className}`;
  const content = (
    <>
      {children}
      {arrow && (
        <ArrowRight
          aria-hidden="true"
          className="h-4 w-4 transition-transform duration-base ease-out group-hover:translate-x-0.5"
        />
      )}
    </>
  );
  if (href.startsWith('#')) {
    return (
      <a href={href} className={classes} onClick={onClick}>
        {content}
      </a>
    );
  }
  return (
    <Link to={href} className={classes} onClick={onClick}>
      {content}
    </Link>
  );
};

/* -------------------------------------------------------------- Provenance */

export const PROVENANCE_STYLES: Record<
  ProvenanceClass,
  { text: string; border: string; stroke: string; dash?: string }
> = {
  observed: { text: 'text-signal-observed', border: 'border-signal-observed/40', stroke: '#EDEDEF' },
  supplied: { text: 'text-signal-supplied', border: 'border-signal-supplied/40', stroke: '#8C9EB3', dash: '1 4' },
  inferred: { text: 'text-signal-accent', border: 'border-signal-accent/50', stroke: '#C8CAD0', dash: '5 5' },
};

export const ProvenanceBadge: React.FC<{ value: ProvenanceClass; withDescription?: boolean }> = ({
  value,
  withDescription = false,
}) => {
  const style = PROVENANCE_STYLES[value];
  return (
    <span
      className={`inline-flex items-center gap-1.5 border px-1.5 py-0.5 font-mono text-micro uppercase ${style.text} ${style.border}`}
      title={PROVENANCE_DESCRIPTION[value]}
    >
      <svg aria-hidden="true" width="14" height="4" viewBox="0 0 14 4">
        <line x1="0" y1="2" x2="14" y2="2" stroke={style.stroke} strokeWidth="1.5" strokeDasharray={style.dash} />
      </svg>
      {PROVENANCE_LABEL[value]}
      {withDescription && <span className="sr-only">: {PROVENANCE_DESCRIPTION[value]}</span>}
    </span>
  );
};

const REVIEW_STYLES: Record<ReviewStatus, string> = {
  verified: 'text-signal-ok',
  pending: 'text-fg-muted',
  flagged: 'text-signal-flag',
};

export const ReviewBadge: React.FC<{ value: ReviewStatus }> = ({ value }) => (
  <span className={`inline-flex items-center gap-1.5 font-mono text-micro uppercase ${REVIEW_STYLES[value]}`}>
    <span aria-hidden="true" className="h-1.5 w-1.5 bg-current" />
    {REVIEW_LABEL[value]}
  </span>
);

export const ProvenanceLegend: React.FC<{ className?: string }> = ({ className = '' }) => (
  <ul className={`flex flex-wrap gap-x-5 gap-y-2 ${className}`} aria-label="Provenance legend">
    {(Object.keys(PROVENANCE_STYLES) as ProvenanceClass[]).map((key) => (
      <li key={key} className="flex items-center gap-2 font-mono text-micro uppercase text-fg-subtle">
        <svg aria-hidden="true" width="22" height="4" viewBox="0 0 22 4">
          <line
            x1="0"
            y1="2"
            x2="22"
            y2="2"
            stroke={PROVENANCE_STYLES[key].stroke}
            strokeWidth="1.5"
            strokeDasharray={PROVENANCE_STYLES[key].dash}
          />
        </svg>
        {PROVENANCE_LABEL[key]}
      </li>
    ))}
  </ul>
);

/* ------------------------------------------------------------------- Brand */

/** Original YUKTI mark: a single red-anchored thread joining three evidence points. */
export const YuktiMark: React.FC<{ className?: string }> = ({ className = 'h-6 w-6' }) => (
  <svg viewBox="0 0 24 24" className={className} aria-hidden="true" fill="none">
    <rect x="0.5" y="0.5" width="23" height="23" stroke="currentColor" strokeOpacity="0.35" />
    <path d="M5 7 C 12 7, 12 12, 12 12 S 12 17, 19 17" stroke="currentColor" strokeWidth="1.5" />
    <rect x="3.5" y="5.5" width="3" height="3" fill="#E5243B" />
    <rect x="10.5" y="10.5" width="3" height="3" fill="currentColor" />
    <rect x="17.5" y="15.5" width="3" height="3" fill="currentColor" />
  </svg>
);

export const YuktiWordmark: React.FC<{ className?: string }> = ({ className = '' }) => (
  <span className={`inline-flex items-center gap-2.5 ${className}`}>
    <YuktiMark className="h-7 w-7 text-fg" />
    <span className="font-serif text-[1.375rem] font-semibold uppercase leading-none tracking-[0.1em] text-fg-bright">Yukti</span>
  </span>
);

/** Thin labelled frame used for product windows and diagrams. */
export const Frame: React.FC<{
  title?: React.ReactNode;
  meta?: React.ReactNode;
  className?: string;
  bodyClassName?: string;
  children: React.ReactNode;
}> = ({ title, meta, className = '', bodyClassName = '', children }) => (
  <div className={`border border-ink-line bg-ink-900 ${className}`}>
    {(title || meta) && (
      <div className="flex items-center justify-between gap-3 border-b border-ink-line px-4 py-2.5 font-mono text-micro uppercase text-fg-subtle">
        <span className="flex min-w-0 items-center gap-2 truncate">{title}</span>
        {meta && <span className="shrink-0">{meta}</span>}
      </div>
    )}
    <div className={bodyClassName}>{children}</div>
  </div>
);
