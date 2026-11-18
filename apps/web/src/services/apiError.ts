/**
 * Failure categories for backend requests:
 * - `network`: the request never produced an HTTP response (DNS, CORS, offline, connection refused).
 * - `http`: the backend answered with a non-2xx status.
 * - `timeout`: the request exceeded its time budget and was aborted.
 * - `parse`: a 2xx response body was not valid JSON or failed runtime validation.
 *
 * Cancellation requested by the caller is NOT an ApiError; it is surfaced as a
 * standard `AbortError` DOMException (see `isAbortError`).
 */
export type ApiErrorKind = 'network' | 'http' | 'timeout' | 'parse';

export interface ApiErrorOptions {
  kind: ApiErrorKind;
  message: string;
  url: string;
  status?: number;
  code?: string;
  details?: unknown;
  cause?: unknown;
}

export class ApiError extends Error {
  readonly kind: ApiErrorKind;
  readonly url: string;
  /** HTTP status code; only set when `kind === 'http'`. */
  readonly status: number | undefined;
  /** Machine-readable error code from the standardized error envelope, when provided. */
  readonly code: string | undefined;
  /** Extra diagnostic payload (envelope `details` or FastAPI validation errors). */
  readonly details: unknown;
  /** Underlying error (e.g. fetch TypeError, SyntaxError). Declared here because the ES2020 lib lacks ErrorOptions. */
  readonly cause: unknown;

  constructor(options: ApiErrorOptions) {
    super(options.message);
    this.name = 'ApiError';
    this.cause = options.cause;
    this.kind = options.kind;
    this.url = options.url;
    this.status = options.status;
    this.code = options.code;
    this.details = options.details;
  }
}

export function isApiError(error: unknown): error is ApiError {
  return error instanceof ApiError;
}

/** True when the error represents a cancelled request (caller-initiated abort). */
export function isAbortError(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    'name' in error &&
    (error as { name: unknown }).name === 'AbortError'
  );
}

export interface ParsedErrorBody {
  message?: string;
  code?: string;
  details?: unknown;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function formatValidationItem(item: unknown): string | undefined {
  if (typeof item === 'string') {
    return item;
  }
  if (!isRecord(item) || typeof item.msg !== 'string') {
    return undefined;
  }
  const loc = Array.isArray(item.loc)
    ? item.loc.filter((part) => typeof part === 'string' || typeof part === 'number').join('.')
    : '';
  return loc ? `${loc}: ${item.msg}` : item.msg;
}

/**
 * Extract a human-readable message from an error response body. Supports:
 * - Standardized envelope: `{"error": {"code": string, "message": string, "details"?: unknown}}`
 * - FastAPI `HTTPException`: `{"detail": string}`
 * - FastAPI validation (422): `{"detail": [{"loc": [...], "msg": string, "type": string}, ...]}`
 * Unknown shapes yield an empty result so callers can fall back to the HTTP status.
 */
export function parseErrorBody(body: unknown): ParsedErrorBody {
  if (!isRecord(body)) {
    return {};
  }

  const envelope = body.error;
  if (isRecord(envelope)) {
    const result: ParsedErrorBody = {};
    if (typeof envelope.message === 'string' && envelope.message.trim() !== '') {
      result.message = envelope.message;
    }
    if (typeof envelope.code === 'string' && envelope.code.trim() !== '') {
      result.code = envelope.code;
    }
    if (envelope.details !== undefined && envelope.details !== null) {
      result.details = envelope.details;
    }
    return result;
  }

  const detail = body.detail;
  if (typeof detail === 'string' && detail.trim() !== '') {
    return { message: detail };
  }
  if (Array.isArray(detail)) {
    const messages = detail
      .map(formatValidationItem)
      .filter((msg): msg is string => msg !== undefined && msg !== '');
    return {
      ...(messages.length > 0 ? { message: messages.join('; ') } : {}),
      details: detail,
    };
  }

  return {};
}

/**
 * Convert any thrown value into a safe, displayable message. Never throws.
 */
export function getErrorMessage(error: unknown, fallback = 'An unexpected error occurred.'): string {
  if (error instanceof Error) {
    return error.message.trim() !== '' ? error.message : fallback;
  }
  if (typeof error === 'string') {
    return error.trim() !== '' ? error : fallback;
  }
  if (isRecord(error) && typeof error.message === 'string' && error.message.trim() !== '') {
    return error.message;
  }
  return fallback;
}
