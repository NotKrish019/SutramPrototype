import { DEMO_CASE, type DemoCase } from '../../../../fixtures/synthetic/case026';

export type WorkbenchSelection =
  | { kind: 'evidence'; id: string }
  | { kind: 'event'; id: string }
  | { kind: 'entity'; id: string };

export interface DerivedSelection {
  evidenceIds: Set<string>;
  eventIds: Set<string>;
  entityIds: Set<string>;
  /** Event to mark as selected in the timeline, if any. */
  selectedEventId: string | null;
  /** Entity to mark as selected in the graph, if any. */
  selectedEntityId: string | null;
}

/**
 * Resolve a selection to everything it is connected to in the case, so every
 * panel highlights the same evidence-backed context.
 */
export function deriveSelection(selection: WorkbenchSelection, data: DemoCase = DEMO_CASE): DerivedSelection {
  const evidenceIds = new Set<string>();
  const eventIds = new Set<string>();
  const entityIds = new Set<string>();
  let selectedEventId: string | null = null;
  let selectedEntityId: string | null = null;

  const addEvidence = (id: string): void => {
    const evidence = data.evidence.find((item) => item.id === id);
    if (!evidence) return;
    evidenceIds.add(evidence.id);
    evidence.eventIds.forEach((eventId) => eventIds.add(eventId));
    evidence.entityIds.forEach((entityId) => entityIds.add(entityId));
  };

  switch (selection.kind) {
    case 'evidence':
      addEvidence(selection.id);
      break;
    case 'event': {
      const event = data.events.find((item) => item.id === selection.id);
      if (event) {
        selectedEventId = event.id;
        addEvidence(event.evidenceId);
        eventIds.add(event.id);
      }
      break;
    }
    case 'entity': {
      selectedEntityId = selection.id;
      entityIds.add(selection.id);
      for (const rel of data.relationships) {
        if (rel.from === selection.id) entityIds.add(rel.to);
        if (rel.to === selection.id) entityIds.add(rel.from);
      }
      for (const evidence of data.evidence) {
        if (evidence.entityIds.includes(selection.id)) {
          evidenceIds.add(evidence.id);
          evidence.eventIds.forEach((eventId) => eventIds.add(eventId));
        }
      }
      break;
    }
  }

  return { evidenceIds, eventIds, entityIds, selectedEventId, selectedEntityId };
}

/** Items that require investigator attention (flagged or pending review). */
export function countReviewItems(data: DemoCase = DEMO_CASE): number {
  return data.evidence.filter((item) => item.review !== 'verified').length;
}
