import React from 'react';
import { Info } from 'lucide-react';

export const GraphLegend: React.FC = () => {
  return (
    <div className="space-y-3 p-4">
      <div>
        <h4 className="mb-2 font-mono text-micro uppercase tracking-wider text-fg-subtle">
          Node Categories
        </h4>
        <div className="grid grid-cols-2 gap-2 font-mono text-micro text-fg-muted sm:grid-cols-4">
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rounded-sm border border-blue-500 bg-blue-950/60" />
            <span>Case</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 border border-cyan-400 bg-cyan-950/60" />
            <span>Evidence</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rounded-full border border-amber-400 bg-amber-950/60" />
            <span>Entity Candidate</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rotate-45 border border-orange-500 bg-orange-950/60" />
            <span>Event Candidate</span>
          </div>
          <div className="flex items-center gap-2">
            <span
              className="inline-block h-0 w-0 border-b-[12px] border-l-[6px] border-r-[6px] border-b-red-500 border-l-transparent border-r-transparent"
              title="Contradiction Lead"
            />
            <span className="text-signal-flag">Contradiction Lead</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rounded-full border border-purple-500 bg-purple-950/60" />
            <span>Extraction Run</span>
          </div>
        </div>
      </div>

      <div className="border-t border-ink-line pt-2">
        <h4 className="mb-2 font-mono text-micro uppercase tracking-wider text-fg-subtle">
          Relationship Semantics
        </h4>
        <div className="grid grid-cols-1 gap-2 font-mono text-micro text-fg-muted sm:grid-cols-3">
          <div className="flex items-center gap-2">
            <svg aria-hidden="true" width="28" height="6">
              <line x1="0" y1="3" x2="28" y2="3" stroke="#EDEDEF" strokeWidth="2" />
            </svg>
            <span>Observed Link</span>
          </div>
          <div className="flex items-center gap-2">
            <svg aria-hidden="true" width="28" height="6">
              <line
                x1="0"
                y1="3"
                x2="28"
                y2="3"
                stroke="#FFB224"
                strokeWidth="1.6"
                strokeDasharray="4 3"
              />
            </svg>
            <span>Inferred Link</span>
          </div>
          <div className="flex items-center gap-2">
            <svg aria-hidden="true" width="28" height="6">
              <line
                x1="0"
                y1="3"
                x2="28"
                y2="3"
                stroke="#EF4444"
                strokeWidth="2"
                strokeDasharray="6 3"
              />
            </svg>
            <span className="text-signal-flag">Contradiction</span>
          </div>
        </div>
      </div>

      <div className="flex items-start gap-2 rounded border border-ink-line bg-ink-900 p-2 text-micro leading-relaxed text-fg-subtle">
        <Info className="mt-0.5 h-3.5 w-3.5 shrink-0 text-signal-amber" />
        <span>
          <strong>Forensic Safeguard:</strong> Knowledge graph links represent recorded assertions
          or algorithmic derivations. They do not constitute conclusive proof of identity,
          culpability, or wrongdoing.
        </span>
      </div>
    </div>
  );
};
