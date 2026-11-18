import { useMemo, useSyncExternalStore } from 'react';

/**
 * Minimal History-API router for the two top-level surfaces (public site and
 * investigator workspace). Deliberately dependency-free; see
 * docs/decisions/0001-frontend-client-side-routing.md.
 */

export const ROUTES = {
  home: '/',
  workspace: '/workspace',
  workspaceSettings: '/workspace/settings',
  workspaceHelp: '/workspace/help',
} as const;

export type RoutePath = (typeof ROUTES)[keyof typeof ROUTES];

const NAVIGATE_EVENT = 'yukti:navigate';

function subscribe(onChange: () => void): () => void {
  window.addEventListener('popstate', onChange);
  window.addEventListener(NAVIGATE_EVENT, onChange);
  return () => {
    window.removeEventListener('popstate', onChange);
    window.removeEventListener(NAVIGATE_EVENT, onChange);
  };
}

/** Normalise a pathname: strip trailing slashes except for the root. */
export function normalizePath(pathname: string): string {
  const trimmed = pathname.replace(/\/+$/, '');
  return trimmed === '' ? '/' : trimmed;
}

function getPathname(): string {
  return normalizePath(window.location.pathname);
}

export function usePathname(): string {
  return useSyncExternalStore(subscribe, getPathname, () => '/');
}

function getSearch(): string {
  return window.location.search;
}

/** Current query string parameters (re-renders on navigation). */
export function useSearchParams(): URLSearchParams {
  const search = useSyncExternalStore(subscribe, getSearch, () => '');
  return useMemo(() => new URLSearchParams(search), [search]);
}

/* ------------------------------------------------------------------ Workspace routes */

export const CASE_SECTIONS = [
  'overview',
  'evidence',
  'correlation',
  'graph',
  'timeline',
  'spatial',
  'contradictions',
  'reports',
] as const;

export type CaseSection = (typeof CASE_SECTIONS)[number];

export type WorkspaceRoute =
  | { kind: 'home' }
  | { kind: 'settings' }
  | { kind: 'help' }
  | { kind: 'case'; caseId: string; section: CaseSection }
  | { kind: 'not-found' };

function isCaseSection(value: string): value is CaseSection {
  return (CASE_SECTIONS as readonly string[]).includes(value);
}

/** Parse a normalised pathname under `/workspace`. Returns null for paths outside the workspace. */
export function matchWorkspaceRoute(pathname: string): WorkspaceRoute | null {
  if (pathname !== ROUTES.workspace && !pathname.startsWith(`${ROUTES.workspace}/`)) return null;
  const parts = pathname.slice(ROUTES.workspace.length).split('/').filter(Boolean).map(decodeURIComponent);
  if (parts.length === 0) return { kind: 'home' };
  if (parts.length === 1 && parts[0] === 'settings') return { kind: 'settings' };
  if (parts.length === 1 && parts[0] === 'help') return { kind: 'help' };
  if (parts[0] === 'cases' && parts[1]) {
    const section = parts[2] ?? 'overview';
    if (parts.length <= 3 && isCaseSection(section)) return { kind: 'case', caseId: parts[1], section };
  }
  return { kind: 'not-found' };
}

/** Build a workspace case URL, optionally with query parameters (e.g. a selection). */
export function caseHref(caseId: string, section: CaseSection = 'overview', params?: Record<string, string>): string {
  const base = `${ROUTES.workspace}/cases/${encodeURIComponent(caseId)}${section === 'overview' ? '' : `/${section}`}`;
  const query = params ? new URLSearchParams(params).toString() : '';
  return query ? `${base}?${query}` : base;
}

/**
 * Navigate within the app. `to` may include a hash (e.g. `/#workflow`), in which
 * case the target element is scrolled into view after the route renders.
 */
export function navigate(to: string, options: { replace?: boolean } = {}): void {
  const url = new URL(to, window.location.origin);
  const previousPath = window.location.pathname;
  const current = previousPath + window.location.search + window.location.hash;
  const next = url.pathname + url.search + url.hash;

  if (current !== next) {
    if (options.replace) {
      window.history.replaceState(null, '', next);
    } else {
      window.history.pushState(null, '', next);
    }
    window.dispatchEvent(new Event(NAVIGATE_EVENT));
  }

  if (url.hash) {
    // Wait for the new route to commit before scrolling to the anchor.
    requestAnimationFrame(() => {
      document.getElementById(decodeURIComponent(url.hash.slice(1)))?.scrollIntoView?.();
    });
  } else if (current !== next && (url.pathname !== previousPath || !options.replace)) {
    // New page: start at the top. Query-only replacements (e.g. selections) keep the scroll position.
    window.scrollTo(0, 0);
  }
}
