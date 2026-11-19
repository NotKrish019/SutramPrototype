import React from 'react';
import { ArrowLeftRight, Cctv, CreditCard, KeyRound, MessagesSquare, type LucideIcon } from 'lucide-react';
import { DEMO_CASE, type EvidenceKind } from '../../../../fixtures/synthetic/case026';
import { ReviewBadge } from '../../../../components/ui/primitives';

export const EVIDENCE_ICONS: Record<EvidenceKind, LucideIcon> = {
  cctv: Cctv,
  'access-log': KeyRound,
  communication: MessagesSquare,
  transaction: CreditCard,
  network: ArrowLeftRight,
};

export interface EvidencePanelProps {
  selectedId: string | null;
  related: ReadonlySet<string>;
  onSelect: (id: string) => void;
}

export const EvidencePanel: React.FC<EvidencePanelProps> = ({ selectedId, related, onSelect }) => (
  <div>
    <h4 className="border-b border-ink-line px-4 py-2.5 font-mono text-micro uppercase text-fg-subtle">
      Evidence · {DEMO_CASE.evidence.length} artifacts
    </h4>
    <ul className="divide-y divide-ink-line">
      {DEMO_CASE.evidence.map((item) => {
        const Icon = EVIDENCE_ICONS[item.kind];
        const selected = item.id === selectedId;
        const lit = related.size === 0 || related.has(item.id);
        return (
          <li key={item.id}>
            <button
              type="button"
              aria-pressed={selected}
              onClick={() => onSelect(item.id)}
              className={`group flex w-full items-start gap-3 px-4 py-3 text-left transition-[background-color,opacity] duration-base focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-accent ${
                selected ? 'bg-signal-accent/[0.08]' : 'hover:bg-white/[0.03]'
              } ${lit ? 'opacity-100' : 'opacity-45'}`}
            >
              <span
                className={`mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center border ${
                  selected ? 'border-signal-accent text-signal-accent' : 'border-ink-strong text-fg-muted'
                }`}
              >
                <Icon aria-hidden="true" className="h-3.5 w-3.5" />
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex items-center justify-between gap-2">
                  <span className="truncate text-sm text-fg">{item.label}</span>
                  <span className="shrink-0 font-mono text-micro text-fg-subtle">{item.id}</span>
                </span>
                <span className="mt-0.5 block truncate font-mono text-micro text-fg-subtle">{item.fileName}</span>
                <span className="mt-1.5 block">
                  <ReviewBadge value={item.review} />
                </span>
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  </div>
);
