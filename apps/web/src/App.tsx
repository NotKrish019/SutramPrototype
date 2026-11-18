import React, { useEffect } from 'react';
import { WorkspaceApp } from './features/workspace/WorkspaceApp';
import { caseSectionLabel } from './features/workspace/layout/navigation';
import { LandingPage } from './pages/LandingPage';
import { NotFoundPage } from './pages/NotFoundPage';
import { ROUTES, matchWorkspaceRoute, usePathname, type WorkspaceRoute } from './router/router';

function workspaceTitle(route: WorkspaceRoute): string {
  switch (route.kind) {
    case 'home':
      return 'Workspace · YUKTI Digital Forensics';
    case 'case':
      return `${route.section === 'overview' ? route.caseId : `${caseSectionLabel(route.section)} · ${route.caseId}`} · YUKTI`;
    case 'settings':
      return 'Settings · YUKTI workspace';
    case 'help':
      return 'Help · YUKTI workspace';
    default:
      return 'Not found · YUKTI workspace';
  }
}

export const App: React.FC = () => {
  const pathname = usePathname();
  const workspaceRoute = matchWorkspaceRoute(pathname);

  useEffect(() => {
    const route = matchWorkspaceRoute(pathname);
    if (route) document.title = workspaceTitle(route);
    else if (pathname === ROUTES.home) document.title = 'YUKTI — Digital Evidence Intelligence & Crime Reconstruction';
    else document.title = 'Not found · YUKTI';
  }, [pathname]);

  if (workspaceRoute) return <WorkspaceApp route={workspaceRoute} />;
  if (pathname === ROUTES.home) return <LandingPage />;
  return <NotFoundPage pathname={pathname} />;
};

export default App;
