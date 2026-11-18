import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { getErrorMessage, isAbortError } from '../../../services/apiError';
import type { CaseDetail, WorkspaceCase } from '../types';
import { syntheticRepository, type WorkspaceRepository } from './repository';

const RepositoryContext = createContext<WorkspaceRepository>(syntheticRepository);

export const WorkspaceDataProvider: React.FC<{ repository?: WorkspaceRepository; children: React.ReactNode }> = ({
  repository = syntheticRepository,
  children,
}) => <RepositoryContext.Provider value={repository}>{children}</RepositoryContext.Provider>;

export function useWorkspaceRepository(): WorkspaceRepository {
  return useContext(RepositoryContext);
}

export type ResourceState<T> =
  | { status: 'loading'; data: null; error: null }
  | { status: 'success'; data: T; error: null }
  | { status: 'error'; data: null; error: string };

export interface Resource<T> {
  state: ResourceState<T>;
  retry: () => void;
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
  return { state, retry };
}

export function useCaseList(): Resource<WorkspaceCase[]> {
  const repository = useWorkspaceRepository();
  return useResource((signal) => repository.listCases(signal), [repository]);
}

export function useCaseDetail(caseId: string): Resource<CaseDetail | null> {
  const repository = useWorkspaceRepository();
  return useResource((signal) => repository.getCaseDetail(caseId, signal), [repository, caseId]);
}
