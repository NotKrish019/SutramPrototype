import { ApiError, isAbortError, parseErrorBody } from './apiError';
import { buildApiUrl, DEFAULT_REQUEST_TIMEOUT_MS } from './config';

export interface RequestOptions {
  /** Caller-owned signal; aborting it rejects with an `AbortError` DOMException. */
  signal?: AbortSignal;
  /** Time budget for the whole request (headers + body), in ms. Defaults to 10s. */
  timeoutMs?: number;
  /** Extra fetch options. `signal` is managed internally and cannot be overridden here. */
  init?: Omit<RequestInit, 'signal'>;
}

/** Runtime validator: returns the typed value or throws describing the mismatch. */
export type ResponseParser<T> = (data: unknown) => T;

function createAbortError(reason: unknown): unknown {
  // Preserve a caller-supplied AbortError reason; otherwise build a standard one.
  if (isAbortError(reason)) {
    return reason;
  }
  return new DOMException('The request was aborted.', 'AbortError');
}

function describeStatus(response: Response): string {
  return response.statusText ? `${response.status} ${response.statusText}` : `${response.status}`;
}

function buildHttpError(response: Response, bodyText: string, url: string): ApiError {
  let body: unknown;
  try {
    body = bodyText === '' ? undefined : JSON.parse(bodyText);
  } catch {
    body = undefined; // Non-JSON error body (e.g. proxy HTML page); fall back to status text.
  }
  const parsed = parseErrorBody(body);
  const statusLabel = `HTTP ${describeStatus(response)}`;
  return new ApiError({
    kind: 'http',
    message: parsed.message ? `${statusLabel}: ${parsed.message}` : statusLabel,
    url,
    status: response.status,
    code: parsed.code,
    details: parsed.details,
  });
}

/**
 * Perform a JSON request against the backend with timeout, cancellation,
 * structured error classification and runtime response validation.
 */
export async function requestJson<T>(
  path: string,
  parse: ResponseParser<T>,
  options: RequestOptions = {},
): Promise<T> {
  const { signal, timeoutMs = DEFAULT_REQUEST_TIMEOUT_MS, init } = options;

  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new RangeError(`timeoutMs must be a positive finite number, received ${timeoutMs}.`);
  }

  const url = buildApiUrl(path);

  if (signal?.aborted) {
    throw createAbortError(signal.reason);
  }

  const controller = new AbortController();
  let timedOut = false;
  const onCallerAbort = (): void => controller.abort(signal?.reason);
  signal?.addEventListener('abort', onCallerAbort, { once: true });
  const timer = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, timeoutMs);

  const classifyTransportError = (error: unknown): unknown => {
    if (timedOut) {
      return new ApiError({
        kind: 'timeout',
        message: `Request to ${url} timed out after ${timeoutMs} ms.`,
        url,
        cause: error,
      });
    }
    if (signal?.aborted) {
      return createAbortError(signal.reason);
    }
    return new ApiError({
      kind: 'network',
      message: `Network error while contacting ${url}. Is the backend running and reachable?`,
      url,
      cause: error,
    });
  };

  try {
    let response: Response;
    let bodyText: string;
    const headers = new Headers(init?.headers);
    if (!headers.has('Accept')) {
      headers.set('Accept', 'application/json');
    }
    try {
      response = await fetch(url, { ...init, headers, signal: controller.signal });
      bodyText = await response.text();
    } catch (error) {
      throw classifyTransportError(error);
    }

    if (!response.ok) {
      throw buildHttpError(response, bodyText, url);
    }

    let data: unknown;
    try {
      data = JSON.parse(bodyText);
    } catch (error) {
      throw new ApiError({
        kind: 'parse',
        message: `Response from ${url} is not valid JSON.`,
        url,
        cause: error,
      });
    }

    try {
      return parse(data);
    } catch (error) {
      const reason = error instanceof Error ? `: ${error.message}` : '.';
      throw new ApiError({
        kind: 'parse',
        message: `Unexpected response shape from ${url}${reason}`,
        url,
        details: data,
        cause: error,
      });
    }
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener('abort', onCallerAbort);
  }
}
