import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { getErrorMessage, isAbortError } from '../../../services/apiError';
import { fetchCases } from '../../../services/casesApi';
import type { CaseDetail, WorkspaceCase } from '../types';
import { liveRepository, syntheticRepository, type WorkspaceRepository } from './repository';
import { useOptionalBackendHealth } from './BackendHealthContext';

const RepositoryContext = createContext<WorkspaceRepository>(syntheticRepository);

/**
 * Without an explicit repository the workspace follows the shared health monitor: live API data
 * (plus the labelled synthetic demo cases) while the Rust API answers, synthetic fixtures otherwise.
 */
export const WorkspaceDataProvider: React.FC<{ repository?: WorkspaceRepository; children: React.ReactNode }> = ({
  repository,
  children,
}) => {
  const caseApi = useCaseApiAvailable();
  const automatic = caseApi ? liveRepository : syntheticRepository;
  return <RepositoryContext.Provider value={repository ?? automatic}>{children}</RepositoryContext.Provider>;
};

export function useWorkspaceRepository(): WorkspaceRepository {
  return useContext(RepositoryContext);
}

/**
 * True once the API's case endpoint has answered with a case list. `/health` alone is not enough:
 * an older or partial API can be healthy without serving cases.
 */
function useCaseApiAvailable(): boolean {
  const health = useOptionalBackendHealth();
  const connected = health?.status === 'connected';
  const [available, setAvailable] = useState(false);
  useEffect(() => {
    if (!connected) {
      setAvailable(false);
      return;
    }
    const controller = new AbortController();
    fetchCases({ signal: controller.signal, timeoutMs: 4000 })
      .then(() => setAvailable(true))
      .catch(() => {
        if (!controller.signal.aborted) setAvailable(false);
      });
    return () => controller.abort();
  }, [connected]);
  return available;
}

/** Whether the records on screen are synthetic: decided per case, falling back to the repository. */
export function useIsSynthetic(item?: { source?: WorkspaceCase['source'] } | null): boolean {
  const repository = useWorkspaceRepository();
  return (item?.source ?? repository.source) === 'synthetic';
}

export type ResourceState<T> =
  | { status: 'loading'; data: null; error: null }
  | { status: 'success'; data: T; error: null }
  | { status: 'error'; data: null; error: string };

export interface Resource<T> {
  state: ResourceState<T>;
  retry: () => void;
  /** Reload in the background, keeping the current data on screen (no loading state). */
  refresh: () => void;
}

/** Load async data with cancellation on unmount / dependency change and a retry action. */
export function useResource<T>(load: (signal: AbortSignal) => Promise<T>, deps: React.DependencyList): Resource<T> {
  const [state, setState] = useState<ResourceState<T>>({ status: 'loading', data: null, error: null });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    setState({ status: 'loading', data: null, error: null });
    load(controller.signal)
      .then((data) => {
        if (!controller.signal.aborted) setState({ status: 'success', data, error: null });
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted && isAbortError(error)) return;
        if (!controller.signal.aborted) {
          setState({ status: 'error', data: null, error: getErrorMessage(error, 'Unable to load data.') });
        }
      });
    return () => controller.abort();
    // `load` is intentionally excluded: callers pass the inputs it depends on via `deps`.
  }, [...deps, attempt]);

  const retry = useCallback(() => setAttempt((value) => value + 1), []);
  const loadRef = React.useRef(load);
  loadRef.current = load;
  const refresh = useCallback(() => {
    const controller = new AbortController();
    loadRef.current(controller.signal)
      .then((data) => setState({ status: 'success', data, error: null }))
      .catch(() => {
        // A failed background refresh keeps what is on screen; the next poll tries again.
      });
  }, []);
  return { state, retry, refresh };
}

export function useCaseList(): Resource<WorkspaceCase[]> {
  const repository = useWorkspaceRepository();
  return useResource((signal) => repository.listCases(signal), [repository]);
}

export function useCaseDetail(caseId: string): Resource<CaseDetail | null> {
  const repository = useWorkspaceRepository();
  return useResource((signal) => repository.getCaseDetail(caseId, signal), [repository, caseId]);
}
