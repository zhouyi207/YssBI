import { createContext, useContext, type ReactNode } from 'react';
import { useLogPanelController, type LogPanelController, type LogPanelVariant } from './useLogPanelController';

const LogPanelContext = createContext<LogPanelController | null>(null);

export function LogPanelProvider({
  variant,
  children,
}: {
  variant: LogPanelVariant;
  children: ReactNode;
}) {
  const controller = useLogPanelController(variant);
  return (
    <LogPanelContext.Provider value={controller}>
      {children}
    </LogPanelContext.Provider>
  );
}

export function useLogPanelContext(): LogPanelController {
  const ctx = useContext(LogPanelContext);
  if (!ctx) {
    throw new Error('useLogPanelContext must be used within LogPanelProvider');
  }
  return ctx;
}
