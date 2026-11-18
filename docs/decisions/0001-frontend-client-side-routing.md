# 0001 — Frontend client-side routing without a router dependency

- **Status:** Proposed (Member 3, 2026-11-18). Pending team review.
- **Scope:** `apps/web/` only. No backend or `shared/` changes.

## Context

The frontend rendered a single page (`AppLayout` + `HomePage`). The public product site needs `/`,
and the existing investigator workspace, with its API client and backend-health monitor, must stay reachable.

## Decision

Add a small, typed History-API router in `apps/web/src/router/` instead of adding `react-router`:

| Path | Renders |
| --- | --- |
| `/` | `LandingPage`, the public product site |
| `/workspace/*` | Investigator workspace — see `docs/architecture/frontend-workspace.md` |
| anything else | `NotFoundPage` with links to `/` and `/workspace` |

- `usePathname()` uses `useSyncExternalStore` over `popstate` plus an internal navigate event.
- `navigate(to)` pushes or replaces history, scrolls to `#hash` targets or to the top.
- `<Link to>` intercepts only plain left clicks. Modified clicks and `target="_blank"` fall through to the browser.
- Trailing slashes are normalised (`/workspace/` is the same as `/workspace`).
- `matchWorkspaceRoute()` parses `/workspace/*` into a typed route; `caseHref()` builds case URLs.
- `useSearchParams()` exposes the query string; selections use `navigate(…, { replace: true })`, which keeps
  the scroll position when only the query changes.

## Rationale

There are two surfaces with one level of workspace sub-routes, one path parameter (`caseId`) and query-string
selections. There are no loaders or nested data routing, so a dependency would add bundle size and API surface
without clear benefit. The router is around 130 lines and covered by `src/router/router.test.tsx`,
`src/App.test.tsx` and `src/features/workspace/workspace.unit.test.ts`.

## Consequences

- Static hosting must rewrite unknown paths to `index.html` (SPA fallback). `vite dev` and `vite preview`
  already do this. Production hosting config (e.g. nginx `try_files`) must do the same.
- If routing needs grow (nested routes, params, data loading), revisit and adopt `react-router` behind the
  same `ROUTES` constants.
