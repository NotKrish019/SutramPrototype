import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ChevronRight, Menu, X } from 'lucide-react';
import { Link } from '../../../router/Link';
import { ROUTES, caseHref, navigate, type WorkspaceRoute } from '../../../router/router';
import { BackendHealthProvider } from '../data/BackendHealthContext';
import { WorkspaceDataProvider, useWorkspaceRepository } from '../data/WorkspaceDataContext';
import type { WorkspaceRepository } from '../data/repository';
import { SyntheticTag } from '../components/ui';
import { BackendConnectionIndicator } from './BackendConnectionIndicator';
import { WorkspaceSearch } from './WorkspaceSearch';
import { WorkspaceShellProvider, useWorkspaceShell } from './WorkspaceShellContext';
import { WorkspaceSidebar } from './WorkspaceSidebar';
import { caseSectionLabel } from './navigation';
import { CliBridge } from '../../cli-bridge/CliBridge';

export const COLLAPSE_KEY = 'yukti.workspace.sidebarCollapsed';
/** Key used before the SUTRAM → YUKTI rename; migrated once so saved preferences survive. */
export const LEGACY_COLLAPSE_KEY = 'sutram.workspace.sidebarCollapsed';

function readCollapsed(): boolean {
  try {
    const current = window.localStorage.getItem(COLLAPSE_KEY);
    if (current !== null) return current === '1';
    const legacy = window.localStorage.getItem(LEGACY_COLLAPSE_KEY);
    if (legacy !== null) {
      window.localStorage.setItem(COLLAPSE_KEY, legacy);
      window.localStorage.removeItem(LEGACY_COLLAPSE_KEY);
    }
    // No saved preference: start as a slim icon rail so the case visuals get the room.
    return legacy === null ? true : legacy === '1';
  } catch {
    return true;
  }
}

function writeCollapsed(value: boolean): void {
  try {
    window.localStorage.setItem(COLLAPSE_KEY, value ? '1' : '0');
  } catch {
    // Storage unavailable (private mode, blocked) — the preference simply isn't remembered.
  }
}

const Breadcrumbs: React.FC = () => {
  const { route } = useWorkspaceShell();
  const crumbs: { label: string; href?: string }[] = [{ label: 'Workspace', href: ROUTES.workspace }];
  if (route.kind === 'case') {
    crumbs.push({ label: route.caseId, href: route.section === 'overview' ? undefined : caseHref(route.caseId) });
    if (route.section !== 'overview') crumbs.push({ label: caseSectionLabel(route.section) });
  } else if (route.kind === 'settings') crumbs.push({ label: 'Settings' });
  else if (route.kind === 'help') crumbs.push({ label: 'Help' });
  else if (route.kind === 'not-found') crumbs.push({ label: 'Not found' });
  else crumbs[0] = { label: 'Workspace' };

  return (
    <nav aria-label="Breadcrumb" className="hidden min-w-0 md:block">
      <ol className="flex min-w-0 items-center gap-1.5 font-mono text-micro uppercase">
        {crumbs.map((crumb, index) => {
          const last = index === crumbs.length - 1;
          return (
            <li key={`${crumb.label}-${index}`} className="flex min-w-0 items-center gap-1.5">
              {index > 0 && <ChevronRight aria-hidden="true" className="h-3 w-3 shrink-0 text-fg-subtle" />}
              {crumb.href && !last ? (
                <Link to={crumb.href} className="truncate text-fg-subtle hover:text-fg">
                  {crumb.label}
                </Link>
              ) : (
                <span aria-current={last ? 'page' : undefined} className="truncate text-fg">
                  {crumb.label}
                </span>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
};

const CaseSelector: React.FC = () => {
  const { route, cases, activeCaseId } = useWorkspaceShell();
  if (cases.state.status !== 'success' || cases.state.data.length === 0) return null;
  const section = route.kind === 'case' ? route.section : 'overview';
  return (
    <label className="hidden h-9 items-center gap-2 border border-ink-line bg-ink-950 pl-2.5 xl:flex">
      <span className="font-mono text-micro uppercase text-fg-subtle">Case</span>
      <select
        value={activeCaseId ?? ''}
        onChange={(event) => navigate(caseHref(event.target.value, section))}
        className="h-full max-w-[15rem] bg-transparent pr-2 text-sm text-fg focus:outline-none"
      >
        {!activeCaseId && <option value="">Select a case</option>}
        {cases.state.data.map((c) => (
          <option key={c.case_id} value={c.case_id} className="bg-ink-900">
            {c.case_id} · {c.title}
          </option>
        ))}
      </select>
    </label>
  );
};

const Topbar: React.FC<{ onOpenMenu: () => void; menuButtonRef: React.RefObject<HTMLButtonElement> }> = ({
  onOpenMenu,
  menuButtonRef,
}) => {
  const repository = useWorkspaceRepository();
  return (
    <header className="sticky top-0 z-40 border-b border-ink-line bg-ink-950/95 backdrop-blur-md">
      <div className="flex h-14 items-center gap-3 px-3 sm:px-4 lg:px-6">
        <button
          ref={menuButtonRef}
          type="button"
          onClick={onOpenMenu}
          aria-label="Open navigation"
          className="inline-flex h-9 w-9 shrink-0 items-center justify-center border border-ink-strong text-fg hover:bg-white/5 focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-accent lg:hidden"
        >
          <Menu aria-hidden="true" className="h-4 w-4" />
        </button>
        <Breadcrumbs />
        <div className="flex min-w-0 flex-1 justify-end gap-2 md:justify-center">
          <WorkspaceSearch />
        </div>
        <CaseSelector />
        {repository.source === 'synthetic' && <SyntheticTag className="hidden 2xl:inline-flex" />}
        <BackendConnectionIndicator />
      </div>
      {repository.source === 'synthetic' && (
        <p className="border-t border-ink-line bg-signal-accent/[0.06] px-3 py-1.5 font-mono text-micro uppercase text-signal-accent sm:px-4 lg:px-6 2xl:hidden">
          Synthetic demonstration data · case records are fictional · backend API: health only
        </p>
      )}
    </header>
  );
};

/** Full-screen surfaces (the reconstruction theatre) render without the sidebar and top bar. */
const CinemaShell: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className="site-root ws-theme min-h-screen bg-ink-950 text-fg">
    <main id="workspace-main" className="min-h-screen">
      {children}
    </main>
    <CliBridge />
  </div>
);

const Shell: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { route } = useWorkspaceShell();
  if (route.kind === 'case' && route.section === 'theatre') return <CinemaShell>{children}</CinemaShell>;
  return <StandardShell>{children}</StandardShell>;
};

const StandardShell: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const drawerRef = useRef<HTMLDivElement>(null);

  const toggleCollapsed = (): void => {
    setCollapsed((value) => {
      writeCollapsed(!value);
      return !value;
    });
  };

  const closeDrawer = useCallback((restoreFocus = false) => {
    setDrawerOpen(false);
    if (restoreFocus) menuButtonRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!drawerOpen) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    drawerRef.current?.querySelector<HTMLElement>('a, button')?.focus();
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') closeDrawer(true);
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.body.style.overflow = previousOverflow;
      document.removeEventListener('keydown', onKey);
    };
  }, [drawerOpen, closeDrawer]);

  return (
    <div className="site-root ws-theme min-h-screen bg-ink-950 text-fg lg:flex">
      <a
        href="#workspace-main"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[70] focus:bg-signal-accent focus:px-4 focus:py-2 focus:font-mono focus:text-xs focus:uppercase focus:text-ink-950"
      >
        Skip to content
      </a>

      <div
        className={`sticky top-0 hidden h-screen shrink-0 border-r border-ink-line bg-ink-950 transition-[width] duration-base ease-out lg:block ${
          collapsed ? 'w-16' : 'w-64'
        }`}
      >
        <WorkspaceSidebar collapsed={collapsed} onToggleCollapsed={toggleCollapsed} />
      </div>

      {drawerOpen && (
        <div className="fixed inset-0 z-[60] lg:hidden" role="dialog" aria-modal="true" aria-label="Workspace navigation">
          <button
            type="button"
            aria-label="Close navigation"
            tabIndex={-1}
            onClick={() => closeDrawer(true)}
            className="absolute inset-0 bg-black/70"
          />
          <div ref={drawerRef} className="relative flex h-full w-[min(18rem,85vw)] flex-col border-r border-ink-strong bg-ink-950">
            <button
              type="button"
              onClick={() => closeDrawer(true)}
              aria-label="Close navigation"
              className="absolute right-2 top-3 z-10 inline-flex h-8 w-8 items-center justify-center text-fg-muted hover:text-fg"
            >
              <X aria-hidden="true" className="h-4 w-4" />
            </button>
            <WorkspaceSidebar collapsed={false} variant="drawer" onNavigate={() => closeDrawer()} />
          </div>
        </div>
      )}

      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar onOpenMenu={() => setDrawerOpen(true)} menuButtonRef={menuButtonRef} />
        <main id="workspace-main" className="ws-canvas min-w-0 flex-1">
          {children}
        </main>
      </div>
      <CliBridge />
    </div>
  );
};

export interface WorkspaceLayoutProps {
  route: WorkspaceRoute;
  /** Override the data source (tests, future API integration). */
  repository?: WorkspaceRepository;
  children: React.ReactNode;
}

export const WorkspaceLayout: React.FC<WorkspaceLayoutProps> = ({ route, repository, children }) => (
  <BackendHealthProvider>
    <WorkspaceDataProvider repository={repository}>
      <WorkspaceShellProvider route={route}>
        <Shell>{children}</Shell>
      </WorkspaceShellProvider>
    </WorkspaceDataProvider>
  </BackendHealthProvider>
);
