import { createContext, useContext, type ReactNode } from "react";
import { useLogWorkspaceController, type LogWorkspaceController } from "@/features/application/log";

const LogWorkspaceContext = createContext<LogWorkspaceController | null>(null);

export interface LogWorkspaceProviderProps {
  readonly children: ReactNode;
}

export function LogWorkspaceProvider({ children }: LogWorkspaceProviderProps) {
  const controller = useLogWorkspaceController();

  return <LogWorkspaceContext.Provider value={controller}>{children}</LogWorkspaceContext.Provider>;
}

export function useLogWorkspaceContext(): LogWorkspaceController {
  const controller = useContext(LogWorkspaceContext);
  if (!controller) {
    throw new Error("useLogWorkspaceContext must be used within LogWorkspaceProvider");
  }
  return controller;
}
