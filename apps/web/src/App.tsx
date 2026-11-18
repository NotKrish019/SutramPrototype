import React, { useEffect } from 'react';
import { WorkspaceApp } from './features/workspace/WorkspaceApp';
import { caseSectionLabel } from './features/workspace/layout/navigation';
import { LandingPage } from './pages/LandingPage';
import { NotFoundPage } from './pages/NotFoundPage';
import { ROUTES, matchWorkspaceRoute, usePathname, type WorkspaceRoute } from './router/router';

function workspaceTitle(route: WorkspaceRoute): string {
  switch (route.kind) {
    case 'home':
      return 'Workspace · SUTRAM Digital Forensics';
    case 'case':
      return `${route.section === 'overview' ? route.caseId : `${caseSectionLabel(route.section)} · ${route.caseId}`} · SUTRAM`;
    case 'settings':
      return 'Settings · SUTRAM workspace';
    case 'help':
      return 'Help · SUTRAM workspace';
    default:
      return 'Not found · SUTRAM workspace';
  }
}

export const App: React.FC = () => {
  const pathname = usePathname();
  const workspaceRoute = matchWorkspaceRoute(pathname);

  useEffect(() => {
    const route = matchWorkspaceRoute(pathname);
    if (route) document.title = workspaceTitle(route);
    else if (pathname === ROUTES.home) document.title = 'SUTRAM — Digital Evidence Intelligence & Crime Reconstruction';
    else document.title = 'Not found · SUTRAM';
  }, [pathname]);

  if (workspaceRoute) return <WorkspaceApp route={workspaceRoute} />;
  if (pathname === ROUTES.home) return <LandingPage />;
  return <NotFoundPage pathname={pathname} />;
};

export default App;
