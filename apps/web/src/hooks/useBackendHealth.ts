import { useEffect, useState } from 'react';
import { fetchHealth } from '../services/api';
import { getErrorMessage, isAbortError, isApiError, type ApiErrorKind } from '../services/apiError';
import type { HealthCheckResponse } from '../types';

export type BackendHealthState =
  | { status: 'loading' }
  | { status: 'success'; data: HealthCheckResponse }
  | { status: 'error'; message: string; kind: ApiErrorKind | 'unknown' };

/**
 * Fetch backend health once on mount. The in-flight request is aborted on
 * unmount, so no state updates occur after the component is gone.
 */
export function useBackendHealth(): BackendHealthState {
  const [state, setState] = useState<BackendHealthState>({ status: 'loading' });

  useEffect(() => {
    const controller = new AbortController();

    fetchHealth({ signal: controller.signal })
      .then((data) => {
        setState({ status: 'success', data });
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted && isAbortError(error)) {
          return; // Cancelled by unmount; nothing to report.
        }
        setState({
          status: 'error',
          message: getErrorMessage(error, 'Backend unreachable.'),
          kind: isApiError(error) ? error.kind : 'unknown',
        });
      });

    return () => controller.abort();
  }, []);

  return state;
}
