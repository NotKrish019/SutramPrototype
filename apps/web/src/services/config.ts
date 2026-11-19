/** Backend URL used when `VITE_API_URL` is unset or blank (local Rust Axum dev server). */
export const DEFAULT_API_BASE_URL = 'http://localhost:8080';

/** Default per-request timeout in milliseconds. */
export const DEFAULT_REQUEST_TIMEOUT_MS = 10_000;

/**
 * Normalise a raw `VITE_API_URL` value into a base URL without a trailing slash.
 * Throws a descriptive error for values that are not absolute http(s) URLs so
 * misconfiguration surfaces immediately instead of producing broken requests.
 */
export function resolveApiBaseUrl(raw: string | undefined): string {
  const trimmed = raw?.trim() ?? '';
  if (trimmed === '') {
    return DEFAULT_API_BASE_URL;
  }

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    throw new Error(`Invalid VITE_API_URL "${trimmed}": expected an absolute http(s) URL.`);
  }

  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error(
      `Invalid VITE_API_URL "${trimmed}": unsupported protocol "${parsed.protocol}".`,
    );
  }

  return trimmed.replace(/\/+$/, '');
}

/** Base URL of the backend API, read from the Vite environment at call time. */
export function getApiBaseUrl(): string {
  return resolveApiBaseUrl(import.meta.env.VITE_API_URL);
}

/** Build an absolute backend URL for an API path such as `/health`. */
export function buildApiUrl(path: string): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${getApiBaseUrl()}${normalizedPath}`;
}
