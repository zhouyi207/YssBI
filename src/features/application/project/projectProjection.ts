import { useProjectIOStore } from "./projectIOStore";
import { useShallow } from "zustand/react/shallow";

export function useGraphLoadStatus(graphPath: string) {
  return useProjectIOStore((state) => state.graphLoadStatus[graphPath]);
}

export interface ProjectProjection {
  readonly status: "idle" | "loading" | "ready" | "error";
  readonly error: { readonly code: string; readonly incidentId: string | null } | null;
  readonly graphLoadStatus: Readonly<Record<string, "loading" | "ready" | "error">>;
  readonly currentPath: string | null;
  readonly projectInstanceId: string | null;
}

export function useProjectProjection(): ProjectProjection {
  return useProjectIOStore(
    useShallow((state) => ({
      status: state.status,
      error: state.error,
      graphLoadStatus: state.graphLoadStatus,
      currentPath: state.currentPath,
      projectInstanceId: state.projectInstanceId,
    })),
  );
}
