import React from 'react';
import { Compass } from 'lucide-react';
import { HomePage } from '../../pages/HomePage';
import { Link } from '../../router/Link';
import { ROUTES, type WorkspaceRoute } from '../../router/router';
import { EmptyState } from './components/ui';
import type { WorkspaceRepository } from './data/repository';
import { WorkspaceLayout } from './layout/WorkspaceLayout';
import { CaseView } from './pages/CaseView';
import { HelpPage, SettingsPage } from './pages/SettingsAndHelp';

const RouteContent: React.FC<{ route: WorkspaceRoute }> = ({ route }) => {
  switch (route.kind) {
    case 'home':
      return <HomePage />;
    case 'case':
      return <CaseView caseId={route.caseId} section={route.section} />;
    case 'settings':
      return <SettingsPage />;
    case 'help':
      return <HelpPage />;
    case 'not-found':
    default:
      return (
        <EmptyState
          icon={Compass}
          title="No workspace view at this path"
          action={
            <Link to={ROUTES.workspace} className="font-mono text-micro uppercase text-signal-amber hover:underline">
              Back to investigations
            </Link>
          }
        />
      );
  }
};

/** The investigator workspace: shell plus the view for the current `/workspace/*` route. */
export const WorkspaceApp: React.FC<{ route: WorkspaceRoute; repository?: WorkspaceRepository }> = ({ route, repository }) => (
  <WorkspaceLayout route={route} repository={repository}>
    <RouteContent route={route} />
  </WorkspaceLayout>
);
