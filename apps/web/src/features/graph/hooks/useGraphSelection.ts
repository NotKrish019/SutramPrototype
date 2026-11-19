import { useCallback, useState } from 'react';
import type { CytoscapeEdgeData, CytoscapeNodeData } from '../types/graph';

export interface GraphSelectionState {
  selectedNode: CytoscapeNodeData | null;
  selectedEdge: CytoscapeEdgeData | null;
  pathTargetId: string;
  selectNode: (node: CytoscapeNodeData | null) => void;
  selectEdge: (edge: CytoscapeEdgeData | null) => void;
  setPathTargetId: (id: string) => void;
  clearSelection: () => void;
}

export function useGraphSelection(): GraphSelectionState {
  const [selectedNode, setSelectedNode] = useState<CytoscapeNodeData | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<CytoscapeEdgeData | null>(null);
  const [pathTargetId, setPathTargetId] = useState<string>('');

  const selectNode = useCallback((node: CytoscapeNodeData | null) => {
    setSelectedEdge(null);
    setSelectedNode(node);
  }, []);

  const selectEdge = useCallback((edge: CytoscapeEdgeData | null) => {
    setSelectedNode(null);
    setSelectedEdge(edge);
    setPathTargetId('');
  }, []);

  const clearSelection = useCallback(() => {
    setSelectedNode(null);
    setSelectedEdge(null);
    setPathTargetId('');
  }, []);

  return {
    selectedNode,
    selectedEdge,
    pathTargetId,
    selectNode,
    selectEdge,
    setPathTargetId,
    clearSelection,
  };
}
