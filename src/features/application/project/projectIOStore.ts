import { createBoundApplicationStore } from "@/features/core/state/applicationStore";
import { LoadStatus } from "@/shared/types/ui/common";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { logger } from "@/features/application/observability/appLogger";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import {
  beginGraphLoadLifecycle,
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
  setCurrentPath: (path) => set({ currentPath: path ? formatDisplayPath(path) : null }),
  loadGraph: async (graphPath) => {
    if (isGraphCachedInMemory(graphPath)) {
      set((state) => ({
        graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: "ready" },
      }));
      return true;
    }

    const existing = loadGraphInFlight.get(graphPath);
    if (existing) return existing.promise;

    set((state) => ({
      graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: "loading" },
    }));
    const lifecycleToken = beginGraphLoadLifecycle(graphPath);
    const pending = loadGraphProjection(graphPath, lifecycleToken)
      .catch((err) => {
        const error = toErrorReference(err, GRAPH_PROJECTION_CONTRACT_ERROR_CODE);
        set({ error });
        try {
          logger.sys.error(`Failed to load graph projection [${error.code}]`, "ProjectIOStore");
        } catch {
          // Observability cannot prevent completion of a failed graph load.
        }
        return false;
      })
      .then((loaded) => {
        const current = loadGraphInFlight.get(graphPath);
        if (current?.lifecycleToken === lifecycleToken && current.promise === pending) {
          set((state) => ({
            graphLoadStatus: {
              ...state.graphLoadStatus,
              [graphPath]: loaded ? "ready" : "error",
            },
          }));
        }
        return loaded;
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
