import type { HealthCheckResponse } from '../types';
import { requestJson, type RequestOptions } from './http';
import { parseHealthCheckResponse } from './validators';

export const HEALTH_PATH = '/health';

/**
 * Fetch backend health. Rejects with an `ApiError` (network/http/timeout/parse),
 * or with an `AbortError` DOMException if `options.signal` is aborted.
 */
export function fetchHealth(
  options: Pick<RequestOptions, 'signal' | 'timeoutMs'> = {},
): Promise<HealthCheckResponse> {
  return requestJson(HEALTH_PATH, parseHealthCheckResponse, options);
}
