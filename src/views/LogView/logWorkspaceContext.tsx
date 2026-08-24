import { createContext, useContext, useMemo, type ReactNode } from 'react';
import type { LogPanelPresentation } from './useLogPanelVirtualList';
import {
  useLogWorkspaceController,
  type LogWorkspaceController,
} from './useLogWorkspaceController';

export interface LogWorkspaceContextValue extends LogWorkspaceController {
  readonly presentation: LogPanelPresentation;
}

const LogWorkspaceContext = createContext<LogWorkspaceContextValue | null>(null);

export interface LogWorkspaceProviderProps {
  readonly children: ReactNode;
  readonly presentation: LogPanelPresentation;
}

export function LogWorkspaceProvider({
  children,
  presentation,
}: LogWorkspaceProviderProps) {
  const controller = useLogWorkspaceController();
  const value = useMemo(
    () => ({ ...controller, presentation }),
    [controller, presentation],
  );

  return (
    <LogWorkspaceContext.Provider value={value}>
      {children}
    </LogWorkspaceContext.Provider>
  );
}

export function useLogWorkspaceContext(): LogWorkspaceContextValue {
  const controller = useContext(LogWorkspaceContext);
  if (!controller) {
    throw new Error('useLogWorkspaceContext must be used within LogWorkspaceProvider');
  }
  return controller;
}
