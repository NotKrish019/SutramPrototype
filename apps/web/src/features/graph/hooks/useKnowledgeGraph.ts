import { useCallback, useEffect, useState } from 'react';
import { getErrorMessage, isAbortError } from '../../../services/apiError';
import {
  adaptBackendNodes,
  adaptBackendRelationships,
} from '../services/graphAdapter';
import {
  fetchEntityNeighbors,
  fetchEntityPath,
  fetchGraphSummary,
} from '../services/graphApi';
import type {
  CytoscapeEdgeData,
  CytoscapeElement,
  CytoscapeNodeData,
  GraphFilterState,
  GraphPathResponseDto,
  GraphSummaryDto,
} from '../types/graph';
import { INITIAL_FILTER_STATE } from '../utils/graphFilters';

export interface KnowledgeGraphHookState {
  summary: GraphSummaryDto | null;
  elements: CytoscapeElement[];
  filteredElements: CytoscapeElement[];
  isLoading: boolean;
  isExpanding: boolean;
  isTracingPath: boolean;
  error: string | null;
  pathResult: GraphPathResponseDto | null;
  filters: GraphFilterState;
  setFilters: React.Dispatch<React.SetStateAction<GraphFilterState>>;
  expandNode: (entityId: string, depth?: number, limit?: number) => Promise<void>;
  tracePath: (fromId: string, toId: string, maxHops?: number) => Promise<boolean>;
  clearPath: () => void;
  refresh: () => void;
}

export function useKnowledgeGraph(
  caseId: string,
  initialElements: CytoscapeElement[] = [],
): KnowledgeGraphHookState {
  const [summary, setSummary] = useState<GraphSummaryDto | null>(null);
  const [elements, setElements] = useState<CytoscapeElement[]>(initialElements);
  const [filters, setFilters] = useState<GraphFilterState>(INITIAL_FILTER_STATE);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [isExpanding, setIsExpanding] = useState<boolean>(false);
  const [isTracingPath, setIsTracingPath] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [pathResult, setPathResult] = useState<GraphPathResponseDto | null>(null);
  const [refreshTrigger, setRefreshTrigger] = useState<number>(0);

  // Sync initialElements when provided or changed
  useEffect(() => {
    if (initialElements.length > 0) {
      setElements(initialElements);
    }
  }, [initialElements]);

  // Load backend summary on mount or refresh
  useEffect(() => {
    const controller = new AbortController();
    setIsLoading(true);
    setError(null);

    fetchGraphSummary(caseId, { signal: controller.signal })
      .then((data) => {
        if (!controller.signal.aborted) {
          setSummary(data);
          setIsLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted && isAbortError(err)) return;
        if (!controller.signal.aborted) {
          // If summary fails, graph might be unconfigured or local filesystem mode;
          // preserve local elements and record degraded status without crashing
          setError(getErrorMessage(err, 'Unable to load knowledge graph summary.'));
          setIsLoading(false);
        }
      });

    return () => controller.abort();
  }, [caseId, refreshTrigger]);

  // Neighborhood expansion via backend
  const expandNode = useCallback(
    async (entityId: string, depth = 1, limit = 50) => {
      setIsExpanding(true);
      setError(null);

      try {
        const res = await fetchEntityNeighbors(caseId, entityId, depth, limit);
        const newNodes = adaptBackendNodes(res.nodes, entityId);
        const newEdges = adaptBackendRelationships(res.relationships);

        setElements((prev) => {
          const existingIds = new Set(prev.map((el) => el.data.id));
          const additions: CytoscapeElement[] = [];

          for (const n of newNodes) {
            if (!existingIds.has(n.data.id)) {
              existingIds.add(n.data.id);
              additions.push(n);
            }
          }

          for (const e of newEdges) {
            if (!existingIds.has(e.data.id)) {
              existingIds.add(e.data.id);
              additions.push(e);
            }
          }

          return [...prev, ...additions];
        });
      } catch (err: unknown) {
        setError(getErrorMessage(err, `Failed to expand neighbors for entity '${entityId}'.`));
      } finally {
        setIsExpanding(false);
      }
    },
    [caseId],
  );

  // Shortest path tracing via backend
  const tracePath = useCallback(
    async (fromId: string, toId: string, maxHops = 3): Promise<boolean> => {
      setIsTracingPath(true);
      setError(null);

      try {
        const res = await fetchEntityPath(caseId, fromId, toId, maxHops);
        setPathResult(res);

        if (res.found) {
          const pathNodeIds = new Set(res.nodes.map((n) => n.id));
          pathNodeIds.add(fromId);
          pathNodeIds.add(toId);
          const pathEdgeIds = new Set(res.relationships.map((r) => r.id));

          setElements((prev) =>
            prev.map((el) => {
              if (el.group === 'nodes') {
                const node = el.data as CytoscapeNodeData;
                return {
                  ...el,
                  data: {
                    ...node,
                    isOnPath: pathNodeIds.has(node.id),
                  },
                };
              } else {
                const edge = el.data as CytoscapeEdgeData;
                return {
                  ...el,
                  data: {
                    ...edge,
                    isOnPath: pathEdgeIds.has(edge.id),
                  },
                };
              }
            }),
          );
          return true;
        } else {
          return false;
        }
      } catch (err: unknown) {
        setError(getErrorMessage(err, 'Failed to trace evidentiary path.'));
        return false;
      } finally {
        setIsTracingPath(false);
      }
    },
    [caseId],
  );

  // Clear path highlights
  const clearPath = useCallback(() => {
    setPathResult(null);
    setElements((prev) =>
      prev.map((el) => ({
        ...el,
        data: {
          ...el.data,
          isOnPath: false,
        },
      })),
    );
  }, []);

  const refresh = useCallback(() => {
    setRefreshTrigger((v) => v + 1);
  }, []);

  // Compute filtered elements
  const visibleNodes = elements.filter(
    (el) =>
      el.group === 'nodes' &&
      !filters.hiddenCategories.has((el.data as CytoscapeNodeData).category) &&
      (!filters.showContradictionsOnly ||
        (el.data as CytoscapeNodeData).category === 'ContradictionLead') &&
      (!filters.searchQuery.trim() ||
        (el.data as CytoscapeNodeData).label
          .toLowerCase()
          .includes(filters.searchQuery.toLowerCase()) ||
        (el.data as CytoscapeNodeData).id
          .toLowerCase()
          .includes(filters.searchQuery.toLowerCase())),
  );

  const visibleNodeIds = new Set(visibleNodes.map((n) => n.data.id));

  const visibleEdges = elements.filter(
    (el) =>
      el.group === 'edges' &&
      visibleNodeIds.has((el.data as CytoscapeEdgeData).source) &&
      visibleNodeIds.has((el.data as CytoscapeEdgeData).target) &&
      !filters.hiddenEdgeTypes.has((el.data as CytoscapeEdgeData).relationType) &&
      (!filters.showOnlyObserved || (el.data as CytoscapeEdgeData).isObserved) &&
      (!filters.showContradictionsOnly || (el.data as CytoscapeEdgeData).isContradiction),
  );

  const filteredElements = [...visibleNodes, ...visibleEdges];

  return {
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
  };
}
