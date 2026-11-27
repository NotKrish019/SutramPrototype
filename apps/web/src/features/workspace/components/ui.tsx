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
  <Tag className={`ws-card min-w-0 ${className}`} aria-labelledby={headingId}>
    {(title || meta || actions) && (
      <div className="flex min-h-[3.25rem] items-center justify-between gap-3 border-b border-white/[0.05] px-5 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          {title && (
            <h2 id={headingId} className="truncate text-[0.95rem] font-semibold tracking-[-0.01em] text-fg-bright">
              {title}
            </h2>
          )}
          {meta && <span className="truncate text-xs text-fg-subtle">{meta}</span>}
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
  <dl className={`divide-y divide-white/[0.05] ${className}`}>
    {items.map(([key, value], index) => (
      <div key={index} className="grid grid-cols-[minmax(7rem,38%)_1fr] gap-3 px-4 py-2.5">
        <dt className="text-xs text-fg-subtle">{key}</dt>
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
  <span className={`inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border px-2.5 py-0.5 text-xs font-medium ${CASE_STATUS_STYLE[status]}`}>
    <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-current" />
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
  <span className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium ${CATEGORY_STYLE[category]}`}>
    {REVIEW_CATEGORY_LABEL[category]}
  </span>
);

export const SyntheticTag: React.FC<{ className?: string }> = ({ className = '' }) => (
  <span
    className={`inline-flex items-center gap-1.5 rounded-full border border-signal-warn/30 bg-signal-warn/10 px-2.5 py-0.5 text-xs font-medium text-signal-warn ${className}`}
  >
    <FlaskConical aria-hidden="true" className="h-3 w-3" />
    Synthetic demonstration data
  </span>
);

/* ------------------------------------------------------------- Async states */

export const LoadingState: React.FC<{ label?: string }> = ({ label = 'Loading' }) => (
  <div role="status" className="flex items-center gap-3 px-5 py-10 text-sm text-fg-subtle">
    <Loader2 aria-hidden="true" className="h-4 w-4 animate-spin motion-reduce:animate-none" />
    {label}…
  </div>
);

export const ErrorState: React.FC<{ message: string; onRetry?: () => void }> = ({ message, onRetry }) => (
  <div role="alert" className="flex flex-col items-start gap-3 px-4 py-8">
    <p className="flex items-center gap-2 text-sm font-medium text-signal-flag">
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
    <span className="flex h-11 w-11 items-center justify-center rounded-xl border border-white/10 bg-white/[0.03] text-fg-subtle">
      <Icon aria-hidden="true" className="h-4 w-4" />
    </span>
    <p className="font-display text-xl font-semibold text-fg-bright">{title}</p>
    {children && <div className="max-w-lg text-sm leading-relaxed text-fg-muted">{children}</div>}
    {action}
  </div>
);

/* ------------------------------------------------------------------ Controls */

type ButtonVariant = 'default' | 'primary' | 'ghost';

const BUTTON_VARIANT: Record<ButtonVariant, string> = {
  default: 'border border-white/10 bg-white/[0.03] text-fg hover:border-white/20 hover:bg-white/[0.06]',
  primary: 'border border-crimson bg-crimson text-white shadow-[0_8px_24px_-10px_rgba(229,36,59,0.8)] hover:bg-crimson-deep',
  ghost: 'border border-transparent text-fg-muted hover:text-fg hover:bg-white/[0.05]',
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
      className={`inline-flex items-center justify-center gap-2 rounded-lg font-medium transition-colors duration-fast focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-accent disabled:cursor-not-allowed disabled:opacity-45 ${
        size === 'sm' ? 'h-8 px-3 text-xs' : 'h-9 px-4 text-sm'
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
    className={`inline-flex h-8 items-center gap-2 rounded-full border px-3 text-xs font-medium transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent ${
      active ? 'border-white/20 bg-white/10 text-fg-bright' : 'border-white/[0.06] text-fg-muted hover:border-white/15 hover:text-fg'
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
      className="h-full w-full rounded-lg border border-white/[0.08] bg-white/[0.03] pl-9 pr-8 text-sm text-fg placeholder:text-fg-subtle focus:border-signal-accent/60 focus:outline-none [&::-webkit-search-cancel-button]:hidden"
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
  <label className={`flex h-9 items-center gap-2 rounded-lg border border-white/[0.08] bg-white/[0.03] pl-3 ${className}`}>
    <span className="shrink-0 text-xs text-fg-subtle">{label}</span>
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
