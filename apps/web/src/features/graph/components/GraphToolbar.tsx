import React from 'react';
import {
  AlertOctagon,
  Eye,
  Filter,
  Maximize2,
  Minus,
  Plus,
  RotateCcw,
  Search,
} from 'lucide-react';
import type { ForensicNodeCategory, GraphFilterState } from '../types/graph';
import { CATEGORY_LABELS } from '../utils/graphFilters';

export interface GraphToolbarProps {
  totalNodes: number;
  totalEdges: number;
  layoutName: string;
  filters: GraphFilterState;
  onLayoutChange: (layout: string) => void;
  onFilterChange: (updater: (prev: GraphFilterState) => GraphFilterState) => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFit: () => void;
  onReset: () => void;
}

export const GraphToolbar: React.FC<GraphToolbarProps> = ({
  totalNodes,
  totalEdges,
  layoutName,
  filters,
  onLayoutChange,
  onFilterChange,
  onZoomIn,
  onZoomOut,
  onFit,
  onReset,
}) => {
  const [showFilterMenu, setShowFilterMenu] = React.useState(false);

  const toggleCategory = (cat: ForensicNodeCategory) => {
    onFilterChange((prev) => {
      const next = new Set(prev.hiddenCategories);
      if (next.has(cat)) {
        next.delete(cat);
      } else {
        next.add(cat);
      }
      return { ...prev, hiddenCategories: next };
    });
  };

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-ink-line bg-ink-900 px-4 py-2.5">
      {/* Left: Search & Telemetry */}
      <div className="flex items-center gap-3">
        <div className="relative">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-fg-subtle" />
          <input
            type="text"
            value={filters.searchQuery}
            onChange={(e) =>
              onFilterChange((prev) => ({ ...prev, searchQuery: e.target.value }))
            }
            placeholder="Search graph…"
            aria-label="Search graph nodes"
            className="h-8 w-48 rounded border border-ink-line bg-ink-950 pl-8 pr-3 font-mono text-xs text-fg placeholder:text-fg-subtle focus:border-signal-amber focus:outline-none focus:ring-1 focus:ring-signal-amber sm:w-64"
          />
        </div>

        <span className="font-mono text-micro uppercase text-fg-subtle">
          {totalNodes} NODES · {totalEdges} LINKS
        </span>
      </div>

      {/* Right: Controls & Layout */}
      <div className="flex flex-wrap items-center gap-1.5">
        {/* Layout Selector */}
        <select
          value={layoutName}
          onChange={(e) => onLayoutChange(e.target.value)}
          aria-label="Graph layout algorithm"
          className="h-8 rounded border border-ink-line bg-ink-950 px-2 font-mono text-xs text-fg focus:border-signal-amber focus:outline-none focus:ring-1 focus:ring-signal-amber"
        >
          <option value="cose">Force-Directed (CoSE)</option>
          <option value="concentric">Concentric Radial</option>
          <option value="circle">Circular</option>
          <option value="breadthfirst">Hierarchical Flow</option>
        </select>

        {/* Quick Toggles */}
        <button
          type="button"
          onClick={() =>
            onFilterChange((prev) => ({ ...prev, showOnlyObserved: !prev.showOnlyObserved }))
          }
          className={`flex h-8 items-center gap-1.5 rounded border px-2.5 font-mono text-xs transition-colors ${
            filters.showOnlyObserved
              ? 'border-signal-amber/40 bg-signal-amber/15 text-signal-amber'
              : 'border-ink-line bg-ink-950 text-fg-subtle hover:text-fg'
          }`}
          title="Filter to only show observed links (hide inferred associations)"
        >
          <Eye className="h-3 w-3" />
          <span className="hidden sm:inline">Observed Only</span>
        </button>

        <button
          type="button"
          onClick={() =>
            onFilterChange((prev) => ({
              ...prev,
              showContradictionsOnly: !prev.showContradictionsOnly,
            }))
          }
          className={`flex h-8 items-center gap-1.5 rounded border px-2.5 font-mono text-xs transition-colors ${
            filters.showContradictionsOnly
              ? 'border-signal-flag/40 bg-signal-flag/15 text-signal-flag'
              : 'border-ink-line bg-ink-950 text-fg-subtle hover:text-fg'
          }`}
          title="Filter to only highlight contradiction leads"
        >
          <AlertOctagon className="h-3 w-3" />
          <span className="hidden sm:inline">Contradictions</span>
        </button>

        {/* Filter Dropdown Toggle */}
        <div className="relative">
          <button
            type="button"
            onClick={() => setShowFilterMenu((v) => !v)}
            aria-expanded={showFilterMenu}
            aria-label="Toggle node category filters"
            className={`flex h-8 items-center gap-1.5 rounded border px-2.5 font-mono text-xs transition-colors ${
              filters.hiddenCategories.size > 0
                ? 'border-signal-cyan/40 bg-signal-cyan/15 text-signal-cyan'
                : 'border-ink-line bg-ink-950 text-fg-subtle hover:text-fg'
            }`}
          >
            <Filter className="h-3 w-3" />
            <span>Categories</span>
          </button>

          {showFilterMenu && (
            <div
              className="absolute right-0 top-full z-50 mt-1 w-56 rounded border border-ink-line bg-ink-900 p-2 shadow-2xl"
              role="dialog"
              aria-label="Category filter menu"
            >
              <p className="mb-1.5 px-1 font-mono text-micro uppercase text-fg-subtle">
                Visible Categories
              </p>
              <div className="space-y-1">
                {(
                  [
                    'Case',
                    'Evidence',
                    'ExtractionRun',
                    'EntityCandidate',
                    'EventCandidate',
                    'ContradictionLead',
                  ] as ForensicNodeCategory[]
                ).map((cat) => {
                  const isVisible = !filters.hiddenCategories.has(cat);
                  return (
                    <label
                      key={cat}
                      className="flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-xs text-fg hover:bg-white/[0.04]"
                    >
                      <input
                        type="checkbox"
                        checked={isVisible}
                        onChange={() => toggleCategory(cat)}
                        className="rounded border-ink-line bg-ink-950 text-signal-amber focus:ring-0"
                      />
                      <span>{CATEGORY_LABELS[cat]}</span>
                    </label>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        <div className="mx-1 h-5 w-px bg-ink-line" />

        {/* View Controls */}
        <button
          type="button"
          onClick={onZoomIn}
          aria-label="Zoom in"
          className="flex h-8 w-8 items-center justify-center rounded border border-ink-line bg-ink-950 text-fg-subtle hover:text-fg"
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          onClick={onZoomOut}
          aria-label="Zoom out"
          className="flex h-8 w-8 items-center justify-center rounded border border-ink-line bg-ink-950 text-fg-subtle hover:text-fg"
        >
          <Minus className="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          onClick={onFit}
          aria-label="Fit graph to view"
          className="flex h-8 w-8 items-center justify-center rounded border border-ink-line bg-ink-950 text-fg-subtle hover:text-fg"
        >
          <Maximize2 className="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          onClick={onReset}
          aria-label="Reset zoom and center"
          className="flex h-8 w-8 items-center justify-center rounded border border-ink-line bg-ink-950 text-fg-subtle hover:text-fg"
        >
          <RotateCcw className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
};
