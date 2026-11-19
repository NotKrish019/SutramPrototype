import React, { useId } from 'react';
import { AlertTriangle, FlaskConical, Loader2, RotateCw, Search, X, type LucideIcon } from 'lucide-react';
import { Eyebrow } from '../../../components/ui/primitives';
import type { CaseStatus } from '../../../types';
import { CASE_STATUS_LABEL, REVIEW_CATEGORY_LABEL } from '../format';
import type { ReviewCategory, WorkspaceEvidence } from '../types';

/* --------------------------------------------------------------- Page chrome */

export const PageHeader: React.FC<{
  eyebrow: string;
  title: React.ReactNode;
  description?: React.ReactNode;
  actions?: React.ReactNode;
  children?: React.ReactNode;
}> = ({ eyebrow, title, description, actions, children }) => (
  <header className="border-b border-ink-line px-4 pb-6 pt-6 sm:px-6 lg:px-8 lg:pt-8">
    <div className="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
      <div className="min-w-0">
        <Eyebrow className="mb-3">{eyebrow}</Eyebrow>
        <h1 className="font-serif text-[clamp(1.9rem,3.2vw,2.75rem)] font-normal leading-[1.02] tracking-[-0.015em] text-fg-bright">
          {title}
        </h1>
        {description && <p className="mt-3 max-w-2xl text-sm leading-relaxed text-fg-muted">{description}</p>}
      </div>
      {actions && <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>}
    </div>
    {children}
  </header>
);

export const Panel: React.FC<{
  title?: React.ReactNode;
  meta?: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
  bodyClassName?: string;
  as?: 'section' | 'div' | 'aside';
  labelledBy?: string;
  children: React.ReactNode;
}> = ({ title, meta, actions, className = '', bodyClassName = '', as: Tag = 'section', labelledBy, children }) => {
  // Every titled panel is a named landmark region, so assistive tech can jump between them.
  const generatedId = useId();
  const headingId = title ? labelledBy ?? generatedId : undefined;
  return (
  <Tag className={`min-w-0 border border-ink-line bg-ink-900 ${className}`} aria-labelledby={headingId}>
    {(title || meta || actions) && (
      <div className="flex min-h-[2.75rem] items-center justify-between gap-3 border-b border-ink-line px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          {title && (
            <h2 id={headingId} className="truncate font-mono text-micro uppercase text-fg-muted">
              {title}
            </h2>
          )}
          {meta && <span className="truncate font-mono text-micro uppercase text-fg-subtle">{meta}</span>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
      </div>
    )}
    <div className={bodyClassName}>{children}</div>
  </Tag>
  );
};

export const KeyValueList: React.FC<{ items: [React.ReactNode, React.ReactNode][]; className?: string }> = ({
  items,
  className = '',
}) => (
  <dl className={`divide-y divide-ink-line ${className}`}>
    {items.map(([key, value], index) => (
      <div key={index} className="grid grid-cols-[minmax(7rem,38%)_1fr] gap-3 px-4 py-2.5">
        <dt className="font-mono text-micro uppercase text-fg-subtle">{key}</dt>
        <dd className="min-w-0 break-words text-sm text-fg">{value}</dd>
      </div>
    ))}
  </dl>
);

/* -------------------------------------------------------------------- Status */

const CASE_STATUS_STYLE: Record<CaseStatus, string> = {
  DRAFT: 'border-ink-strong text-fg-muted',
  ACTIVE: 'border-signal-ok/50 text-signal-ok',
  UNDER_REVIEW: 'border-signal-accent/50 text-signal-accent',
  ARCHIVED: 'border-ink-strong text-fg-subtle',
};

export const CaseStatusBadge: React.FC<{ status: CaseStatus }> = ({ status }) => (
  <span className={`inline-flex items-center gap-1.5 whitespace-nowrap border px-1.5 py-0.5 font-mono text-micro uppercase ${CASE_STATUS_STYLE[status]}`}>
    <span aria-hidden="true" className="h-1.5 w-1.5 bg-current" />
    {CASE_STATUS_LABEL[status]}
  </span>
);

export const EvidenceTypeLabel: React.FC<{ evidence: Pick<WorkspaceEvidence, 'source_type' | 'source_type_label'> }> = ({ evidence }) =>
  evidence.source_type ? (
    <span className="font-mono text-xs text-fg">{evidence.source_type}</span>
  ) : (
    <span className="inline-flex items-center gap-1 font-mono text-xs text-fg-muted" title={evidence.source_type_label}>
      <span className="text-signal-accent">—</span> Unclassified
      <span className="sr-only">: {evidence.source_type_label}</span>
    </span>
  );

const CATEGORY_STYLE: Record<ReviewCategory, string> = {
  conflicting: 'border-signal-flag/60 text-signal-flag',
  uncertain: 'border-signal-accent/60 text-signal-accent',
  missing: 'border-ink-strong text-fg-muted border-dashed',
  unverified: 'border-signal-supplied/60 text-signal-supplied',
  supporting: 'border-signal-ok/50 text-signal-ok',
};

export const ReviewCategoryBadge: React.FC<{ category: ReviewCategory }> = ({ category }) => (
  <span className={`inline-flex items-center border px-1.5 py-0.5 font-mono text-micro uppercase ${CATEGORY_STYLE[category]}`}>
    {REVIEW_CATEGORY_LABEL[category]}
  </span>
);

export const SyntheticTag: React.FC<{ className?: string }> = ({ className = '' }) => (
  <span
    className={`inline-flex items-center gap-1.5 border border-signal-accent/50 bg-signal-accent/10 px-1.5 py-0.5 font-mono text-micro uppercase text-signal-accent ${className}`}
  >
    <FlaskConical aria-hidden="true" className="h-3 w-3" />
    Synthetic demonstration data
  </span>
);

/* ------------------------------------------------------------- Async states */

export const LoadingState: React.FC<{ label?: string }> = ({ label = 'Loading' }) => (
  <div role="status" className="flex items-center gap-3 px-4 py-10 font-mono text-micro uppercase text-fg-subtle">
    <Loader2 aria-hidden="true" className="h-4 w-4 animate-spin motion-reduce:animate-none" />
    {label}…
  </div>
);

export const ErrorState: React.FC<{ message: string; onRetry?: () => void }> = ({ message, onRetry }) => (
  <div role="alert" className="flex flex-col items-start gap-3 px-4 py-8">
    <p className="flex items-center gap-2 font-mono text-micro uppercase text-signal-flag">
      <AlertTriangle aria-hidden="true" className="h-3.5 w-3.5" />
      Could not load data
    </p>
    <p className="text-sm text-fg-muted">{message}</p>
    {onRetry && (
      <WsButton icon={RotateCw} onClick={onRetry}>
        Retry
      </WsButton>
    )}
  </div>
);

export const EmptyState: React.FC<{
  icon: LucideIcon;
  title: string;
  children?: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
}> = ({ icon: Icon, title, children, action, className = '' }) => (
  <div className={`flex flex-col items-start gap-3 px-5 py-10 sm:px-8 ${className}`}>
    <span className="flex h-9 w-9 items-center justify-center border border-ink-strong text-fg-subtle">
      <Icon aria-hidden="true" className="h-4 w-4" />
    </span>
    <p className="font-display text-xl font-semibold uppercase text-fg">{title}</p>
    {children && <div className="max-w-lg text-sm leading-relaxed text-fg-muted">{children}</div>}
    {action}
  </div>
);

/* ------------------------------------------------------------------ Controls */

type ButtonVariant = 'default' | 'primary' | 'ghost';

const BUTTON_VARIANT: Record<ButtonVariant, string> = {
  default: 'border border-ink-strong text-fg hover:border-fg-muted hover:bg-white/[0.03]',
  primary: 'border border-signal-accent bg-signal-accent text-ink-950 hover:bg-[#E9D7B8]',
  ghost: 'border border-transparent text-fg-muted hover:text-fg hover:bg-white/[0.04]',
};

export interface WsButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  icon?: LucideIcon;
  variant?: ButtonVariant;
  size?: 'sm' | 'md';
}

export const WsButton = React.forwardRef<HTMLButtonElement, WsButtonProps>(function WsButton(
  { icon: Icon, variant = 'default', size = 'md', className = '', children, type = 'button', ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={`inline-flex items-center justify-center gap-2 font-mono uppercase tracking-[0.04em] transition-colors duration-fast focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-accent disabled:cursor-not-allowed disabled:opacity-45 ${
        size === 'sm' ? 'h-8 px-2.5 text-micro' : 'h-9 px-3.5 text-[0.75rem]'
      } ${BUTTON_VARIANT[variant]} ${className}`}
      {...rest}
    >
      {Icon && <Icon aria-hidden="true" className="h-3.5 w-3.5 shrink-0" />}
      {children}
    </button>
  );
});

export const FilterChip: React.FC<{
  active: boolean;
  onClick: () => void;
  count?: number;
  children: React.ReactNode;
}> = ({ active, onClick, count, children }) => (
  <button
    type="button"
    aria-pressed={active}
    onClick={onClick}
    className={`inline-flex h-8 items-center gap-2 border px-2.5 font-mono text-micro uppercase transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent ${
      active ? 'border-signal-accent/60 bg-signal-accent/10 text-fg' : 'border-ink-line text-fg-muted hover:border-ink-strong hover:text-fg'
    }`}
  >
    {children}
    {count !== undefined && <span className={active ? 'text-signal-accent' : 'text-fg-subtle'}>{count}</span>}
  </button>
);

export const SearchField: React.FC<{
  value: string;
  onChange: (value: string) => void;
  label: string;
  placeholder?: string;
  className?: string;
}> = ({ value, onChange, label, placeholder, className = '' }) => (
  <label className={`relative flex h-9 items-center ${className}`}>
    <span className="sr-only">{label}</span>
    <Search aria-hidden="true" className="pointer-events-none absolute left-3 h-3.5 w-3.5 text-fg-subtle" />
    <input
      type="search"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      placeholder={placeholder}
      className="h-full w-full border border-ink-line bg-ink-950 pl-9 pr-8 text-sm text-fg placeholder:text-fg-subtle focus:border-signal-accent/60 focus:outline-none [&::-webkit-search-cancel-button]:hidden"
    />
    {value && (
      <button
        type="button"
        aria-label={`Clear ${label.toLowerCase()}`}
        onClick={() => onChange('')}
        className="absolute right-2 p-1 text-fg-subtle hover:text-fg"
      >
        <X aria-hidden="true" className="h-3.5 w-3.5" />
      </button>
    )}
  </label>
);

export const SelectField: React.FC<{
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  className?: string;
}> = ({ label, value, onChange, options, className = '' }) => (
  <label className={`flex h-9 items-center gap-2 border border-ink-line bg-ink-950 pl-3 ${className}`}>
    <span className="shrink-0 font-mono text-micro uppercase text-fg-subtle">{label}</span>
    <select
      value={value}
      onChange={(event) => onChange(event.target.value)}
      className="h-full min-w-0 flex-1 bg-transparent pr-2 text-sm text-fg focus:outline-none"
    >
      {options.map((option) => (
        <option key={option.value} value={option.value} className="bg-ink-900">
          {option.label}
        </option>
      ))}
    </select>
  </label>
);
