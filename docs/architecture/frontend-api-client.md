# Frontend API Client (`apps/web/src/services/`)

Owner: Member 3 (Frontend & Visualization). Frontend only; no backend or `shared/` changes.

## Configuration

| Variable       | Where                                   | Default                 |
| -------------- | --------------------------------------- | ----------------------- |
| `VITE_API_URL` | `apps/web/.env.local` (see `.env.example`) | `http://localhost:8080` (the Rust API) |

- Vite only loads `.env*` files from `apps/web/`. The repository-root `.env` does not reach the frontend.
- `services/config.ts` trims the value, strips trailing slashes, and rejects anything that is not an
  absolute `http(s)` URL with a descriptive error. Blank or unset falls back to the default.
- `VITE_*` values are embedded in the browser bundle. Never put secrets in them.
- Typed via `ImportMetaEnv` in `src/vite-env.d.ts`.

## Module layout

| File                     | Responsibility                                                                 |
| ------------------------ | ------------------------------------------------------------------------------ |
| `services/config.ts`     | Base URL resolution, default timeout (10 s)                                    |
| `services/apiError.ts`   | `ApiError`, error-body parsing, `isAbortError`, safe `getErrorMessage`         |
| `services/http.ts`       | `requestJson(path, parser, { signal, timeoutMs, init })`, the shared transport |
| `services/validators.ts` | Runtime validators for response contracts (`parseHealthCheckResponse`)         |
| `services/api.ts`        | Endpoint functions (`fetchHealth`)                                             |
| `hooks/useBackendHealth.ts` | Health monitoring: polling, retry, visibility pause, cancellation           |
| `components/BackendStatusCard.tsx` | Presentational connection status card (no network access)           |

## Error model

Every failure is an `ApiError` with a `kind`:

| `kind`    | Meaning                                                    | Extra fields                     |
| --------- | ---------------------------------------------------------- | -------------------------------- |
| `network` | No HTTP response (offline, DNS, CORS, connection refused)  | `cause`                          |
| `http`    | Non-2xx status                                             | `status`, `code?`, `details?`    |
| `timeout` | Exceeded `timeoutMs` (default 10 s; covers headers + body) | `cause`                          |
| `parse`   | 2xx body is not JSON or fails runtime validation           | `details` (raw payload), `cause` |

HTTP error bodies are parsed in both formats the backend currently emits:

- Standardized envelope: `{"error": {"code", "message", "details?"}}` (backend 500 handler)
- FastAPI default: `{"detail": "..."}` or a 422 list `{"detail": [{"loc", "msg", "type"}]}`

Non-JSON or unrecognized bodies fall back to `HTTP <status> <statusText>`.

**Cancellation is not an `ApiError`.** When the caller's `AbortSignal` fires, the promise rejects with a
standard `AbortError` `DOMException`. Callers check it with `isAbortError()` and usually ignore it.
Configuration errors (invalid `VITE_API_URL`) and invalid `timeoutMs` (`RangeError`) are thrown as
ordinary errors. UI code should always render messages through `getErrorMessage(unknown)`, which never
throws.

## Backend health monitoring

`useBackendHealth({ intervalMs = 15_000, timeoutMs? })` returns
`{ status, data, error, lastSuccessAt, lastCheckedAt, isChecking, retry }`.

- **States:** `checking` (before the first result and during a manual retry), `connected`, `offline`.
  Background polls keep the current status until they settle, so the UI doesn't flicker.
- **Polling:** a `setTimeout` chain. The next check is scheduled only after the current one settles,
  so requests never overlap. `retry()` and visibility checks are ignored while a request is in flight.
- **Manual retry:** `retry()` checks immediately and restarts the 15 s interval.
- **Tab visibility:** polling stops while `document.visibilityState === 'hidden'`, and a check runs
  immediately when the tab becomes visible.
- **Cleanup:** each effect run owns one `AbortController` and a `disposed` flag. Unmount (and the
  StrictMode double-mount) aborts the in-flight request, clears timers and ignores late responses.
- `data` and `lastSuccessAt` keep the last successful values while offline.

## Adding an endpoint

1. Add the TypeScript type in `src/types/`. Mirror the backend schema and don't edit `shared/` unilaterally.
2. Add a `parseX(data: unknown): X` validator in `services/validators.ts` with tests.
3. Add `fetchX(options)` in `services/api.ts` calling `requestJson('/path', parseX, options)`.
4. Consume it from a hook that passes an `AbortController` signal and aborts on cleanup.
