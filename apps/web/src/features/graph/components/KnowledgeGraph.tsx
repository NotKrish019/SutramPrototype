import React, { useMemo, useRef, useState } from 'react';
import { AlertTriangle, List, Network } from 'lucide-react';
import type { CaseDetail } from '../../workspace/types';
import { useGraphSelection } from '../hooks/useGraphSelection';
import { useKnowledgeGraph } from '../hooks/useKnowledgeGraph';
import { adaptCaseDetailToCytoscape } from '../services/graphAdapter';
import type { CytoscapeEdgeData, CytoscapeNodeData } from '../types/graph';
import { CATEGORY_LABELS } from '../utils/graphFilters';
import { GraphCanvas, type GraphCanvasHandle } from './GraphCanvas';
import { GraphEdgeDetails } from './GraphEdgeDetails';
import { GraphEmptyState } from './GraphEmptyState';
import { GraphLegend } from './GraphLegend';
import { GraphNodeDetails } from './GraphNodeDetails';
import { GraphSyncStatus } from './GraphSyncStatus';
import { GraphToolbar } from './GraphToolbar';

export interface KnowledgeGraphProps {
  detail: CaseDetail;
}

export const KnowledgeGraph: React.FC<KnowledgeGraphProps> = ({ detail }) => {
  const caseId = detail.case.case_id;
  const canvasRef = useRef<GraphCanvasHandle>(null);

  // Generate initial elements from local case details (synthetic or cached)
  const initialElements = useMemo(() => adaptCaseDetailToCytoscape(detail), [detail]);

  const {
    summary,
    elements,
    filteredElements,
    isLoading,
    isExpanding,
    isTracingPath,
    error,
    pathResult,
    filters,
    setFilters,
    expandNode,
    tracePath,
    clearPath,
    refresh,
  } = useKnowledgeGraph(caseId, initialElements);

  const {
    selectedNode,
    selectedEdge,
    pathTargetId,
    selectNode,
    selectEdge,
    setPathTargetId,
    clearSelection,
  } = useGraphSelection();

  const [layoutName, setLayoutName] = useState<string>('cose');
  const [viewMode, setViewMode] = useState<'graph' | 'table'>('graph');

  const visibleNodes = useMemo(
    () =>
      filteredElements
        .filter((el) => el.group === 'nodes')
        .map((el) => el.data as CytoscapeNodeData),
    [filteredElements],
  );

  const visibleEdges = useMemo(
    () =>
      filteredElements
        .filter((el) => el.group === 'edges')
        .map((el) => el.data as CytoscapeEdgeData),
    [filteredElements],
  );

  const nodeMap = useMemo(
    () => new Map(elements.filter((el) => el.group === 'nodes').map((el) => [el.data.id, el.data as CytoscapeNodeData])),
    [elements],
  );

  const handleLayoutChange = (name: string) => {
    setLayoutName(name);
    canvasRef.current?.applyLayout(name);
  };

  const handleResetFilters = () => {
    setFilters({
      hiddenCategories: new Set(),
      hiddenEdgeTypes: new Set(),
      searchQuery: '',
      showOnlyObserved: false,
      showContradictionsOnly: false,
    });
  };

  const isDegraded = summary?.graph_synchronization_status === 'GRAPH_DISABLED';

  return (
    <div className="flex flex-col gap-4 p-4 sm:p-6 lg:p-8">
      {/* Top Header: Case & Telemetry */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-ink-line pb-4">
        <div>
          <h2 className="font-display text-xl font-bold uppercase tracking-tight text-fg">
            Forensic Knowledge Graph
          </h2>
          <p className="font-mono text-micro text-fg-subtle">
            Topological link analysis, entity neighborhoods, and contradiction discovery.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <GraphSyncStatus summary={summary} isLoading={isLoading} onRefresh={refresh} />

          {/* View Mode Toggle (Canvas vs Accessible Table) */}
          <div className="flex rounded border border-ink-line bg-ink-900 p-0.5">
            <button
              type="button"
              onClick={() => setViewMode('graph')}
              aria-label="Interactive graph canvas view"
              className={`flex items-center gap-1.5 rounded px-2.5 py-1 font-mono text-micro uppercase transition-colors ${
                viewMode === 'graph'
                  ? 'bg-ink-800 text-fg shadow-sm'
                  : 'text-fg-subtle hover:text-fg'
              }`}
            >
              <Network className="h-3 w-3" />
              <span>Canvas</span>
            </button>
            <button
              type="button"
              onClick={() => setViewMode('table')}
              aria-label="Accessible tabular graph view"
              className={`flex items-center gap-1.5 rounded px-2.5 py-1 font-mono text-micro uppercase transition-colors ${
                viewMode === 'table'
                  ? 'bg-ink-800 text-fg shadow-sm'
                  : 'text-fg-subtle hover:text-fg'
              }`}
            >
              <List className="h-3 w-3" />
              <span>Table</span>
            </button>
          </div>
        </div>
      </div>

      {/* Error / Degraded Banner if present */}
      {error && !isDegraded && (
        <div className="flex items-center gap-2 rounded border border-signal-amber/30 bg-signal-amber/10 p-3 text-xs text-signal-amber">
          <AlertTriangle className="h-4 w-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Main Graph Grid */}
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-12">
        {/* Left Canvas Panel (8 Cols) */}
        <div className="flex flex-col rounded border border-ink-line bg-ink-900 shadow-xl xl:col-span-8">
          <GraphToolbar
            totalNodes={visibleNodes.length}
            totalEdges={visibleEdges.length}
            layoutName={layoutName}
            filters={filters}
            onLayoutChange={handleLayoutChange}
            onFilterChange={setFilters}
            onZoomIn={() => canvasRef.current?.zoomIn()}
            onZoomOut={() => canvasRef.current?.zoomOut()}
            onFit={() => canvasRef.current?.fit()}
            onReset={() => canvasRef.current?.resetView()}
          />

          <div className="relative min-h-[550px] flex-1">
            {viewMode === 'graph' ? (
              visibleNodes.length === 0 ? (
                <GraphEmptyState
                  isLoading={isLoading}
                  isDegraded={isDegraded}
                  hasFiltersApplied={filters.searchQuery.length > 0 || filters.hiddenCategories.size > 0}
                  onResetFilters={handleResetFilters}
                  onRetry={refresh}
                />
              ) : (
                <GraphCanvas
                  ref={canvasRef}
                  elements={filteredElements}
                  selectedNodeId={selectedNode?.id}
                  selectedEdgeId={selectedEdge?.id}
                  layoutName={layoutName}
                  onSelectNode={(node) => selectNode(node)}
                  onSelectEdge={(edge) => selectEdge(edge)}
                  onClearSelection={clearSelection}
                />
              )
            ) : (
              /* Accessible Tabular Alternative for Assistive Tech */
              <div className="overflow-x-auto p-4">
                <h3 className="mb-2 font-mono text-xs uppercase text-fg-subtle">
                  Graph Nodes List ({visibleNodes.length})
                </h3>
                <table className="w-full text-left font-mono text-xs">
                  <thead className="border-b border-ink-line text-fg-subtle">
                    <tr>
                      <th className="py-2">Category</th>
                      <th className="py-2">Label</th>
                      <th className="py-2">Sublabel / Type</th>
                      <th className="py-2">Confidence</th>
                      <th className="py-2">Action</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-ink-line/40">
                    {visibleNodes.map((n) => (
                      <tr key={n.id} className="hover:bg-white/[0.02]">
                        <td className="py-2 text-signal-amber">{CATEGORY_LABELS[n.category] || n.category}</td>
                        <td className="py-2 font-medium text-fg">{n.label}</td>
                        <td className="py-2 text-fg-subtle">{n.sublabel}</td>
                        <td className="py-2">
                          {n.confidence !== null && n.confidence !== undefined
                            ? `${(n.confidence * 100).toFixed(0)}%`
                            : '—'}
                        </td>
                        <td className="py-2">
                          <button
                            type="button"
                            onClick={() => {
                              selectNode(n);
                              setViewMode('graph');
                              canvasRef.current?.centerOnNode(n.id);
                            }}
                            className="text-signal-cyan hover:underline"
                          >
                            Inspect
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          <div className="border-t border-ink-line bg-ink-950">
            <GraphLegend />
          </div>
        </div>

        {/* Right Inspector Panel (4 Cols) */}
        <aside
          aria-label="Graph Inspector"
          className="flex flex-col rounded border border-ink-line bg-ink-900 shadow-xl xl:col-span-4"
        >
          <div className="border-b border-ink-line bg-ink-850 px-4 py-3">
            <h3 className="font-mono text-micro uppercase tracking-wider text-fg-subtle">
              Forensic Inspector
            </h3>
          </div>

          <div className="flex-1 overflow-y-auto">
            {selectedNode ? (
              <GraphNodeDetails
                caseId={caseId}
                node={selectedNode}
                availableNodes={elements}
                isExpanding={isExpanding}
                isTracingPath={isTracingPath}
                pathTargetId={pathTargetId}
                onSelectPathTarget={setPathTargetId}
                onExpandNeighbors={expandNode}
                onTracePath={tracePath}
                onClearPath={clearPath}
                pathFound={pathResult?.found}
              />
            ) : selectedEdge ? (
              <GraphEdgeDetails
                caseId={caseId}
                edge={selectedEdge}
                sourceLabel={nodeMap.get(selectedEdge.source)?.label}
                targetLabel={nodeMap.get(selectedEdge.target)?.label}
              />
            ) : (
              <div className="p-6 text-center">
                <Network className="mx-auto h-8 w-8 text-fg-subtle opacity-40" />
                <p className="mt-3 font-mono text-xs uppercase text-fg">No Object Selected</p>
                <p className="mt-1 font-mono text-micro text-fg-subtle">
                  Tap or click any node or relationship link in the graph canvas to inspect
                  its provenance, attributes, and supporting evidence.
                </p>
              </div>
            )}
          </div>
        </aside>
      </div>
    </div>
  );
};
