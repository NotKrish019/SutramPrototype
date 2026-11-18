import React, { createContext, useContext } from 'react';
import { useBackendHealth, type UseBackendHealthResult } from '../../../hooks/useBackendHealth';

const BackendHealthContext = createContext<UseBackendHealthResult | null>(null);

/**
 * Runs the existing health monitor once for the whole workspace so the top-bar
 * indicator and the overview card share one poller (no duplicate requests).
 */
export const BackendHealthProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const health = useBackendHealth();
  return <BackendHealthContext.Provider value={health}>{children}</BackendHealthContext.Provider>;
};

export function useSharedBackendHealth(): UseBackendHealthResult {
  const value = useContext(BackendHealthContext);
  if (!value) throw new Error('useSharedBackendHealth must be used inside <BackendHealthProvider>.');
  return value;
}
