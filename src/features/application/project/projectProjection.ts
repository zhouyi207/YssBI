import { useProjectIOStore } from "./projectIOStore";

export interface ProjectProjection {
  readonly status: "idle" | "loading" | "ready" | "error";
  readonly error: { readonly code: string; readonly incidentId: string | null } | null;
  readonly graphLoadStatus: Readonly<Record<string, "loading" | "ready" | "error">>;
  readonly currentPath: string | null;
  readonly projectInstanceId: string | null;
}

export function useProjectProjection(): ProjectProjection {
  return useProjectIOStore((state) => ({
    status: state.status,
    error: state.error,
    graphLoadStatus: state.graphLoadStatus,
    currentPath: state.currentPath,
    projectInstanceId: state.projectInstanceId,
  }));
}

export function getProjectProjection(): ProjectProjection {
  const state = useProjectIOStore.getState();
  return {
    status: state.status,
    error: state.error,
    graphLoadStatus: state.graphLoadStatus,
    currentPath: state.currentPath,
    projectInstanceId: state.projectInstanceId,
  };
}
