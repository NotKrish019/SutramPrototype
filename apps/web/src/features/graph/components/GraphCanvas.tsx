import React, { useEffect, useImperativeHandle, useRef } from 'react';
import cytoscape, { type Core, type LayoutOptions } from 'cytoscape';
import type {
  CytoscapeEdgeData,
  CytoscapeElement,
  CytoscapeNodeData,
} from '../types/graph';
import { forensicGraphStyles } from '../utils/graphStyles';

export interface GraphCanvasHandle {
  zoomIn: () => void;
  zoomOut: () => void;
  fit: () => void;
  resetView: () => void;
  applyLayout: (layoutName: string) => void;
  centerOnNode: (nodeId: string) => void;
}

export interface GraphCanvasProps {
  elements: CytoscapeElement[];
  selectedNodeId?: string | null;
  selectedEdgeId?: string | null;
  layoutName?: string;
  onSelectNode: (node: CytoscapeNodeData | null) => void;
  onSelectEdge: (edge: CytoscapeEdgeData | null) => void;
  onClearSelection: () => void;
}

export const GraphCanvas = React.forwardRef<GraphCanvasHandle, GraphCanvasProps>(
  (
    {
      elements,
      selectedNodeId,
      selectedEdgeId,
      layoutName = 'cose',
      onSelectNode,
      onSelectEdge,
      onClearSelection,
    },
    ref,
  ) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const cyRef = useRef<Core | null>(null);

    // Expose control actions via ref
    useImperativeHandle(ref, () => ({
      zoomIn: () => {
        if (cyRef.current) {
          const cy = cyRef.current;
          cy.zoom({ level: cy.zoom() * 1.25, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
        }
      },
      zoomOut: () => {
        if (cyRef.current) {
          const cy = cyRef.current;
          cy.zoom({ level: cy.zoom() * 0.8, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
        }
      },
      fit: () => {
        if (cyRef.current) {
          cyRef.current.fit(undefined, 40);
        }
      },
      resetView: () => {
        if (cyRef.current) {
          cyRef.current.reset();
          cyRef.current.fit(undefined, 40);
        }
      },
      applyLayout: (name: string) => {
        if (cyRef.current) {
          const layout = cyRef.current.layout(getLayoutConfig(name));
          layout.run();
        }
      },
      centerOnNode: (nodeId: string) => {
        if (cyRef.current) {
          const node = cyRef.current.$id(nodeId);
          if (node.length > 0) {
            cyRef.current.center(node);
          }
        }
      },
    }));

    function getLayoutConfig(name: string): LayoutOptions {
      switch (name) {
        case 'concentric':
          return {
            name: 'concentric',
            animate: true,
            animationDuration: 300,
            spacingFactor: 1.5,
          } as LayoutOptions;
        case 'circle':
          return {
            name: 'circle',
            animate: true,
            animationDuration: 300,
          } as LayoutOptions;
        case 'breadthfirst':
          return {
            name: 'breadthfirst',
            directed: true,
            animate: true,
            animationDuration: 300,
            spacingFactor: 1.5,
          } as LayoutOptions;
        case 'cose':
        default:
          return {
            name: 'cose',
            animate: false, // fast deterministic rendering in test environments
            componentSpacing: 60,
            nodeRepulsion: () => 400000,
            nodeOverlap: 20,
            idealEdgeLength: () => 100,
          } as LayoutOptions;
      }
    }

    // Initialize Cytoscape core
    useEffect(() => {
      if (!containerRef.current) return;

      let cy: Core | null = null;
      try {
        cy = cytoscape({
          container: containerRef.current,
          elements: [],
          style: forensicGraphStyles,
          boxSelectionEnabled: false,
          autounselectify: false,
          minZoom: 0.15,
          maxZoom: 3.5,
        });
      } catch {
        try {
          cy = cytoscape({
            headless: true,
            elements: [],
            style: forensicGraphStyles,
          });
        } catch {
          cy = null;
        }
      }

      if (!cy) return;

      // Events
      cy.on('tap', 'node', (evt) => {
        const data = evt.target.data() as CytoscapeNodeData;
        onSelectNode(data);
      });

      cy.on('tap', 'edge', (evt) => {
        const data = evt.target.data() as CytoscapeEdgeData;
        onSelectEdge(data);
      });

      cy.on('tap', (evt) => {
        if (evt.target === cy) {
          onClearSelection();
        }
      });

      cyRef.current = cy;

      // Resize observer
      let resizeObserver: ResizeObserver | null = null;
      if (typeof ResizeObserver !== 'undefined') {
        resizeObserver = new ResizeObserver(() => {
          cy.resize();
        });
        resizeObserver.observe(containerRef.current);
      }

      return () => {
        resizeObserver?.disconnect();
        cy.removeAllListeners();
        cy.destroy();
        cyRef.current = null;
      };
    }, []);

    // Synchronize elements whenever elements change
    useEffect(() => {
      const cy = cyRef.current;
      if (!cy) return;

      cy.batch(() => {
        const currentIds = new Set(cy.elements().map((el) => el.id()));
        const nextIds = new Set(elements.map((el) => el.data.id));

        // Remove elements no longer present
        cy.elements().forEach((el) => {
          if (!nextIds.has(el.id())) {
            cy.remove(el);
          }
        });

        // Add or update elements
        const elementsToAdd: CytoscapeElement[] = [];
        for (const el of elements) {
          if (currentIds.has(el.data.id)) {
            // Update data attributes without moving node position
            cy.$id(el.data.id).data(el.data);
          } else {
            elementsToAdd.push(el);
          }
        }

        if (elementsToAdd.length > 0) {
          cy.add(elementsToAdd);
          const layout = cy.layout(getLayoutConfig(layoutName));
          layout.run();
        }
      });
    }, [elements, layoutName]);

    // Synchronize selection highlighting
    useEffect(() => {
      const cy = cyRef.current;
      if (!cy) return;

      cy.batch(() => {
        cy.elements().unselect();
        cy.elements().removeClass('dimmed highlighted');

        if (selectedNodeId) {
          const selected = cy.$id(selectedNodeId);
          if (selected.length > 0) {
            selected.select();
            const neighborhood = selected.neighborhood().add(selected);
            cy.elements().addClass('dimmed');
            neighborhood.removeClass('dimmed').addClass('highlighted');
          }
        } else if (selectedEdgeId) {
          const selected = cy.$id(selectedEdgeId);
          if (selected.length > 0) {
            selected.select();
            const connectedNodes = selected.connectedNodes().add(selected);
            cy.elements().addClass('dimmed');
            connectedNodes.removeClass('dimmed').addClass('highlighted');
          }
        }
      });
    }, [selectedNodeId, selectedEdgeId]);

    return (
      <div className="relative h-full min-h-[500px] w-full overflow-hidden bg-ink-950">
        <div
          aria-hidden="true"
          className="pattern-grid pointer-events-none absolute inset-0 opacity-40"
        />
        <div
          ref={containerRef}
          className="h-full w-full cursor-grab active:cursor-grabbing"
          role="region"
          aria-label="Interactive forensic knowledge graph canvas. Use arrow keys, pan, or inspector to explore nodes and links."
          tabIndex={0}
        />
      </div>
    );
  },
);

GraphCanvas.displayName = 'GraphCanvas';
