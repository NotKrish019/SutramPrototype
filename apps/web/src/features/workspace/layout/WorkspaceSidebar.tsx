import React from 'react';
import { ArrowUpRight, PanelLeftClose, PanelLeftOpen } from 'lucide-react';
import { SutramMark } from '../../../components/ui/primitives';
import { Link } from '../../../router/Link';
import { ROUTES, caseHref } from '../../../router/router';
import { useWorkspaceShell } from './WorkspaceShellContext';
import { CASE_NAV, SECONDARY_NAV, WORKSPACE_HOME_NAV } from './navigation';

export interface WorkspaceSidebarProps {
  collapsed: boolean;
  onToggleCollapsed?: () => void;
  /** Called after a navigation item is chosen (closes the mobile drawer). */
  onNavigate?: () => void;
  /** Rendered inside the mobile drawer. */
  variant?: 'rail' | 'drawer';
}

const itemClass = (active: boolean, collapsed: boolean): string =>
  `group relative flex h-9 items-center gap-3 text-sm transition-colors duration-fast focus:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-signal-amber ${
    collapsed ? 'justify-center px-0' : 'px-3'
  } ${active ? 'bg-white/[0.05] text-fg' : 'text-fg-muted hover:bg-white/[0.03] hover:text-fg'}`;

const ActiveBar: React.FC<{ active: boolean }> = ({ active }) => (
  <span
    aria-hidden="true"
    className={`absolute inset-y-1.5 left-0 w-0.5 bg-signal-amber transition-opacity duration-base ${active ? 'opacity-100' : 'opacity-0'}`}
  />
);

const SectionLabel: React.FC<{ collapsed: boolean; children: React.ReactNode }> = ({ collapsed, children }) =>
  collapsed ? (
    <div aria-hidden="true" className="mx-3 my-3 h-px bg-ink-line" />
  ) : (
    <p className="px-3 pb-2 pt-5 font-mono text-micro uppercase text-fg-subtle">{children}</p>
  );

export const WorkspaceSidebar: React.FC<WorkspaceSidebarProps> = ({
  collapsed,
  onToggleCollapsed,
  onNavigate,
  variant = 'rail',
}) => {
  const { route, cases, activeCaseId } = useWorkspaceShell();
  const activeCase = cases.state.status === 'success' ? cases.state.data.find((c) => c.case_id === activeCaseId) : undefined;
  const homeActive = route.kind === 'home';

  return (
    <nav aria-label="Workspace" className="flex h-full flex-col">
      <div className={`flex h-14 shrink-0 items-center border-b border-ink-line ${collapsed ? 'justify-center' : 'justify-between px-3'}`}>
        {/* The whole logo + wordmark returns to the public homepage (client-side, no reload). */}
        <Link
          to={ROUTES.home}
          onClick={onNavigate}
          aria-label="Go to SUTRAM homepage"
          title="Go to SUTRAM homepage"
          className="group -m-1 flex cursor-pointer items-center gap-2.5 p-1 transition-colors duration-fast hover:bg-white/[0.04] focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-amber"
        >
          <SutramMark className="h-7 w-7 text-fg transition-colors duration-fast group-hover:text-signal-amber" />
          {!collapsed && (
            <span className="leading-none">
              <span className="block font-display text-lg font-bold uppercase tracking-[0.08em] text-fg">Sutram</span>
              <span className="block font-mono text-[0.625rem] uppercase tracking-[0.1em] text-fg-subtle transition-colors duration-fast group-hover:text-fg-muted">Investigation workspace</span>
            </span>
          )}
        </Link>
      </div>

      <div className="flex-1 overflow-y-auto overflow-x-hidden py-2">
        <SectionLabel collapsed={collapsed}>Workspace</SectionLabel>
        <Link
          to={WORKSPACE_HOME_NAV.href}
          onClick={onNavigate}
          aria-current={homeActive ? 'page' : undefined}
          title={collapsed ? WORKSPACE_HOME_NAV.label : undefined}
          aria-label={collapsed ? WORKSPACE_HOME_NAV.label : undefined}
          className={itemClass(homeActive, collapsed)}
        >
          <ActiveBar active={homeActive} />
          <WORKSPACE_HOME_NAV.icon aria-hidden="true" className="h-4 w-4 shrink-0" />
          {!collapsed && WORKSPACE_HOME_NAV.label}
        </Link>

        {activeCaseId && (
          <>
            <SectionLabel collapsed={collapsed}>
              <span className="text-fg-muted">{activeCaseId}</span>
              {activeCase && <span className="mt-1 block truncate normal-case tracking-normal text-fg-subtle">{activeCase.title}</span>}
            </SectionLabel>
            <ul>
              {CASE_NAV.map((item) => {
                const active = route.kind === 'case' && route.caseId === activeCaseId && route.section === item.section;
                const badge =
                  item.section === 'contradictions' && activeCase && activeCase.has_detail && activeCase.review_count > 0
                    ? activeCase.review_count
                    : null;
                return (
                  <li key={item.section}>
                    <Link
                      to={caseHref(activeCaseId, item.section)}
                      onClick={onNavigate}
                      aria-current={active ? 'page' : undefined}
                      title={collapsed ? item.label : undefined}
                      aria-label={collapsed ? item.label : undefined}
                      className={itemClass(active, collapsed)}
                    >
                      <ActiveBar active={active} />
                      <item.icon aria-hidden="true" className={`h-4 w-4 shrink-0 ${active ? 'text-signal-amber' : ''}`} />
                      {!collapsed && <span className="flex-1 truncate">{item.label}</span>}
                      {badge !== null && (
                        <span
                          className={
                            collapsed
                              ? 'absolute right-2 top-1.5 h-1.5 w-1.5 bg-signal-flag'
                              : 'font-mono text-micro text-signal-flag'
                          }
                          aria-label={`${badge} items need review`}
                        >
                          {collapsed ? '' : badge}
                        </span>
                      )}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </>
        )}
      </div>

      <div className="shrink-0 border-t border-ink-line py-2">
        <ul>
          {SECONDARY_NAV.map((item) => {
            const active =
              (item.href === ROUTES.workspaceSettings && route.kind === 'settings') ||
              (item.href === ROUTES.workspaceHelp && route.kind === 'help');
            const common = {
              title: collapsed ? item.label : undefined,
              'aria-label': collapsed ? item.label : undefined,
              className: itemClass(active, collapsed),
            };
            const content = (
              <>
                <ActiveBar active={active} />
                <item.icon aria-hidden="true" className="h-4 w-4 shrink-0" />
                {!collapsed && <span className="flex-1">{item.label}</span>}
                {!collapsed && item.external && <ArrowUpRight aria-hidden="true" className="h-3 w-3 text-fg-subtle" />}
              </>
            );
            return (
              <li key={item.label}>
                {item.external ? (
                  <a href={item.href} target="_blank" rel="noopener noreferrer" {...common}>
                    {content}
                    <span className="sr-only">(opens in a new tab)</span>
                  </a>
                ) : (
                  <Link to={item.href} onClick={onNavigate} aria-current={active ? 'page' : undefined} {...common}>
                    {content}
                  </Link>
                )}
              </li>
            );
          })}
        </ul>
        {variant === 'rail' && onToggleCollapsed && (
          <button
            type="button"
            onClick={onToggleCollapsed}
            aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
            aria-expanded={!collapsed}
            className={`${itemClass(false, collapsed)} mt-1 w-full`}
          >
            {collapsed ? (
              <PanelLeftOpen aria-hidden="true" className="h-4 w-4" />
            ) : (
              <>
                <PanelLeftClose aria-hidden="true" className="h-4 w-4" />
                <span className="font-mono text-micro uppercase">Collapse</span>
              </>
            )}
          </button>
        )}
      </div>
    </nav>
  );
};
