import { enforceGraphDocumentCacheLimit } from "@/features/application/editor/graphDocumentCachePolicy";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { createBoundApplicationStore } from "@/features/core/state/applicationStore";
import { LoadStatus } from "@/shared/types/ui/common";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { logger } from "@/utils/frontendLogger";
import {
  beginGraphLoadLifecycle,
  isGraphLifecycleCurrent,
  isGraphUnloading,
  loadGraphProjection,
} from "@/features/application/graphProjection/graphProjectionLifecycle";
import { isGraphCachedInMemory } from "@/features/core/dataStore/graphDocumentLoadPolicy";
import { setProjectPathForViewport } from "@/features/core/viewport/projectPath";

export type GraphLoadStatus = "loading" | "ready" | "error";
export const GRAPH_PROJECTION_CONTRACT_ERROR_CODE = "graph_projection_contract_error";

interface ProjectIOStore {
  status: LoadStatus;
  error: ErrorReference | null;
  graphLoadStatus: Record<string, GraphLoadStatus>;
  currentPath: string | null;
  /** Identity of the installed projection; activation may already target its replacement. */
  projectInstanceId: string | null;
  setCurrentPath(path: string | null): void;
  loadGraph(graphPath: string): Promise<boolean>;
}

interface GraphLoadInFlight {
  lifecycleToken: number;
  promise: Promise<boolean>;
}
const loadGraphInFlight = new Map<string, GraphLoadInFlight>();
export function invalidateGraphLoadOwnership(graphPath: string): void {
  loadGraphInFlight.delete(graphPath);
}
export function resetGraphLoadOwnership(): void {
  loadGraphInFlight.clear();
}

export const useProjectIOStore = createBoundApplicationStore<ProjectIOStore>((set) => ({
  status: LoadStatus.Idle,
  error: null,
  graphLoadStatus: {},
  currentPath: null,
  projectInstanceId: null,
  setCurrentPath: (path) => set({ currentPath: path || null }),
  loadGraph: async (graphPath) => {
    const project = captureProjectLifecycleState();
    if (!isGraphUnloading(graphPath) && isGraphCachedInMemory(graphPath)) {
      set((state) =>
        state.graphLoadStatus[graphPath] === "ready"
          ? state
          : {
              graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: "ready" },
            },
      );
      return isProjectLifecycleStateCurrent(project);
    }

    const existing = loadGraphInFlight.get(graphPath);
    if (existing) return existing.promise;

    set((state) => ({
      graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: "loading" },
    }));
    if (!isProjectLifecycleStateCurrent(project)) return false;
    const lifecycleToken = beginGraphLoadLifecycle(graphPath);
    const isCurrent = () =>
      isProjectLifecycleStateCurrent(project) && isGraphLifecycleCurrent(graphPath, lifecycleToken);
    const pending = loadGraphProjection(graphPath, lifecycleToken)
      .catch((err) => {
        if (!isCurrent()) return false;
        const error = toErrorReference(err, GRAPH_PROJECTION_CONTRACT_ERROR_CODE);
        set({ error });
        if (!isCurrent()) return false;
        try {
          logger.sys.error(`Failed to load graph projection [${error.code}]`, "ProjectIOStore");
        } catch {
          // Observability cannot prevent completion of a failed graph load.
        }
        return false;
      })
      .then(async (loaded) => {
        if (!isCurrent()) return false;
        const current = loadGraphInFlight.get(graphPath);
        if (current?.lifecycleToken === lifecycleToken && current.promise === pending) {
          set((state) => ({
            graphLoadStatus: {
              ...state.graphLoadStatus,
              [graphPath]: loaded ? "ready" : "error",
            },
          }));
          if (!isCurrent()) return false;
          if (loaded) {
            try {
              await enforceGraphDocumentCacheLimit();
            } catch {
              if (!isCurrent()) return false;
              logger.graph.warn("Graph cache cleanup failed", "ProjectIOStore");
            }
          }
        }
        return loaded && isCurrent();
      })
      .finally(() => {
        const current = loadGraphInFlight.get(graphPath);
        if (current?.lifecycleToken === lifecycleToken && current.promise === pending) {
          loadGraphInFlight.delete(graphPath);
        }
      });

    loadGraphInFlight.set(graphPath, { lifecycleToken, promise: pending });
    return pending;
  },
}));

setProjectPathForViewport(useProjectIOStore.getState().currentPath);
useProjectIOStore.subscribe((state) => setProjectPathForViewport(state.currentPath));

/** Capture a read against the displayed projection, which may lag activation. */
export function captureProjectReadContext(projectInstanceId: string | null) {
  const identity = captureProjectLifecycleState();
  if (
    !projectInstanceId ||
    identity.projectInstanceId !== projectInstanceId ||
    useProjectIOStore.getState().projectInstanceId !== projectInstanceId
  ) {
    return null;
  }

  return {
    projectInstanceId,
    isCurrent: () =>
      isProjectLifecycleStateCurrent(identity) &&
      useProjectIOStore.getState().projectInstanceId === projectInstanceId,
  };
}
