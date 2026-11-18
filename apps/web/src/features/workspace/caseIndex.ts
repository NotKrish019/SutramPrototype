import { useCallback, useMemo } from 'react';
import { navigate, useSearchParams } from '../../router/router';
import type { CaseDetail, WorkspaceEntity, WorkspaceEvent, WorkspaceEvidence } from './types';

export interface CaseIndex {
  evidence: Map<string, WorkspaceEvidence>;
  entities: Map<string, WorkspaceEntity>;
  events: Map<string, WorkspaceEvent>;
  /** Display name for any id (evidence, entity or event). */
  nameOf: (id: string) => string;
}

export function useCaseIndex(detail: CaseDetail): CaseIndex {
  return useMemo(() => {
    const evidence = new Map(detail.evidence.map((item) => [item.evidence_id, item]));
    const entities = new Map(detail.entities.map((item) => [item.entity_id, item]));
    const events = new Map(detail.events.map((item) => [item.event_id, item]));
    const nameOf = (id: string): string =>
      entities.get(id)?.canonical_name ?? events.get(id)?.title ?? evidence.get(id)?.original_filename ?? id;
    return { evidence, entities, events, nameOf };
  }, [detail]);
}

/**
 * A selection stored in the URL query (e.g. `?evidence=EV-02`) so it can be deep-linked
 * from other views and survives refresh. Updates replace the history entry.
 */
export function useQuerySelection(key: string): [string | null, (value: string | null) => void] {
  const params = useSearchParams();
  const value = params.get(key);
  const setValue = useCallback(
    (next: string | null) => {
      const url = new URL(window.location.href);
      if (next === null) url.searchParams.delete(key);
      else url.searchParams.set(key, next);
      navigate(`${url.pathname}${url.search}`, { replace: true });
    },
    [key],
  );
  return [value, setValue];
}
