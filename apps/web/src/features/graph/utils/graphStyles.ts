import type cytoscape from 'cytoscape';

export const MONO_FONT = 'JetBrains Mono, ui-monospace, monospace';

/**
 * Forensic Cytoscape Stylesheet.
 * Enforces visual distinction across the 10 Phase 2A node categories and edge types.
 */
export const forensicGraphStyles: cytoscape.StylesheetStyle[] = [
  // Base Node Style
  {
    selector: 'node',
    style: {
      label: 'data(label)',
      'font-family': MONO_FONT,
      'font-size': '11px',
      'text-valign': 'bottom',
      'text-margin-y': 6,
      color: '#EDEDEF',
      'background-color': '#0F0F11',
      'border-width': 2,
      'border-color': '#6E6E77',
      width: 32,
      height: 32,
      'text-max-width': '120px',
      'text-wrap': 'ellipsis',
      'transition-property': 'background-color, border-color, border-width, opacity',
      'transition-duration': 0.2,
    },
  },

  // Node Category Stylings
  {
    selector: 'node[category = "Case"]',
    style: {
      shape: 'round-rectangle',
      'border-color': '#3B82F6', // Blue
      width: 40,
      height: 40,
    },
  },
  {
    selector: 'node[category = "Evidence"]',
    style: {
      shape: 'cut-rectangle',
      'border-color': '#06B6D4', // Cyan
      width: 34,
      height: 34,
    },
  },
  {
    selector: 'node[category = "ExtractionRun"]',
    style: {
      shape: 'ellipse',
      'border-color': '#8B5CF6', // Purple
      width: 28,
      height: 28,
    },
  },
  {
    selector: 'node[category = "EntityCandidate"]',
    style: {
      shape: 'ellipse',
      'border-color': '#EAB308', // Amber
      width: 32,
      height: 32,
    },
  },
  {
    selector: 'node[category = "EventCandidate"]',
    style: {
      shape: 'diamond',
      'border-color': '#F97316', // Orange
      width: 32,
      height: 32,
    },
  },
  {
    selector: 'node[category = "RelationshipAssertion"]',
    style: {
      shape: 'round-diamond',
      'border-color': '#A855F7',
      width: 30,
      height: 30,
    },
  },
  {
    selector: 'node[category = "ReconstructionRun"]',
    style: {
      shape: 'octagon',
      'border-color': '#EC4899',
      width: 32,
      height: 32,
    },
  },
  {
    selector: 'node[category = "ContradictionLead"]',
    style: {
      shape: 'triangle',
      'border-color': '#EF4444', // Red
      'background-color': '#2A1215',
      width: 36,
      height: 36,
    },
  },
  {
    selector: 'node[category = "TemporalCluster"]',
    style: {
      shape: 'round-rectangle',
      'border-color': '#A3A3AB',
      'border-style': 'dashed',
    },
  },
  {
    selector: 'node[category = "SpatialLocus"]',
    style: {
      shape: 'hexagon',
      'border-color': '#14B8A6', // Teal
      width: 32,
      height: 32,
    },
  },

  // State modifiers
  {
    selector: 'node[?isRoot]',
    style: {
      'border-width': 4,
      'border-color': '#FFB224',
      'background-color': '#1E170E',
    },
  },
  {
    selector: 'node[?isOnPath]',
    style: {
      'border-width': 3,
      'border-color': '#FFB224',
      'background-color': '#2A2112',
    },
  },
  {
    selector: 'node[?isRedacted]',
    style: {
      'border-style': 'dashed',
    },
  },
  {
    selector: 'node:selected',
    style: {
      'border-width': 4,
      'border-color': '#FFB224',
      'background-color': '#2A2112',
    },
  },

  // Base Edge Style
  {
    selector: 'edge',
    style: {
      'curve-style': 'bezier',
      'target-arrow-shape': 'triangle',
      'target-arrow-color': '#6E6E77',
      'line-color': '#6E6E77',
      width: 1.5,
      'font-family': MONO_FONT,
      'font-size': '9px',
      color: '#A3A3AB',
      'text-rotation': 'autorotate',
      'text-margin-y': -8,
      label: 'data(label)',
      'transition-property': 'line-color, width, opacity',
      'transition-duration': 0.2,
    },
  },

  // Edge Semantic Modifiers
  {
    selector: 'edge[relationType = "COMMUNICATED_WITH"]',
    style: {
      'line-color': '#EDEDEF',
      'target-arrow-color': '#EDEDEF',
      width: 2,
    },
  },
  {
    selector: 'edge[?isContradiction]',
    style: {
      'line-color': '#EF4444',
      'target-arrow-color': '#EF4444',
      'line-style': 'dashed',
      width: 2.5,
    },
  },
  {
    selector: 'edge[provenance = "inferred"]',
    style: {
      'line-style': 'dashed',
      'line-color': '#FFB224',
      'target-arrow-color': '#FFB224',
    },
  },
  {
    selector: 'edge[provenance = "structural"]',
    style: {
      'line-style': 'dotted',
      'line-color': '#4B5563',
      opacity: 0.6,
    },
  },
  {
    selector: 'edge[?isOnPath]',
    style: {
      'line-color': '#FFB224',
      'target-arrow-color': '#FFB224',
      width: 3.5,
      'z-index': 99,
    },
  },
  {
    selector: 'edge:selected',
    style: {
      'line-color': '#FFB224',
      'target-arrow-color': '#FFB224',
      width: 3,
      'z-index': 99,
    },
  },

  // Dimmed element state
  {
    selector: '.dimmed',
    style: {
      opacity: 0.15,
    },
  },
  {
    selector: '.highlighted',
    style: {
      opacity: 1,
    },
  },
];
