import React, { createContext, useContext, useEffect, useState } from 'react';
import type { WorkspaceRoute } from '../../../router/router';
import { useCaseList, useWorkspaceRepository, type Resource } from '../data/WorkspaceDataContext';
import type { WorkspaceCase } from '../types';

interface WorkspaceShellValue {
  route: WorkspaceRoute;
  cases: Resource<WorkspaceCase[]>;
  /** Case shown in the sidebar's case section: the open case, or the last one opened. */
  activeCaseId: string | null;
}

const ShellContext = createContext<WorkspaceShellValue | null>(null);

export const WorkspaceShellProvider: React.FC<{ route: WorkspaceRoute; children: React.ReactNode }> = ({
  route,
  children,
}) => {
  const repository = useWorkspaceRepository();
  const cases = useCaseList();
  const routeCaseId = route.kind === 'case' ? route.caseId : null;
  const [lastCaseId, setLastCaseId] = useState<string | null>(routeCaseId ?? repository.defaultCaseId);

  useEffect(() => {
    if (routeCaseId) setLastCaseId(routeCaseId);
  }, [routeCaseId]);

  return (
    <ShellContext.Provider value={{ route, cases, activeCaseId: routeCaseId ?? lastCaseId }}>
      {children}
    </ShellContext.Provider>
  );
};

export function useWorkspaceShell(): WorkspaceShellValue {
  const value = useContext(ShellContext);
  if (!value) throw new Error('useWorkspaceShell must be used inside <WorkspaceShellProvider>.');
  return value;
}
