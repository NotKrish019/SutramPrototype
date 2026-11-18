# Frontend API Client (`apps/web/src/services/`)

Owner: Member 3 (Frontend & Visualization). Frontend only; no backend or `shared/` changes.

## Configuration

| Variable       | Where                                   | Default                 |
| -------------- | --------------------------------------- | ----------------------- |
| `VITE_API_URL` | `apps/web/.env.local` (see `.env.example`) | `http://localhost:8000` |

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
| `hooks/useBackendHealth.ts` | React state and cancellation on unmount                                     |

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

## Adding an endpoint

1. Add the TypeScript type in `src/types/`. Mirror the backend schema and don't edit `shared/` unilaterally.
2. Add a `parseX(data: unknown): X` validator in `services/validators.ts` with tests.
3. Add `fetchX(options)` in `services/api.ts` calling `requestJson('/path', parseX, options)`.
4. Consume it from a hook that passes an `AbortController` signal and aborts on cleanup.
