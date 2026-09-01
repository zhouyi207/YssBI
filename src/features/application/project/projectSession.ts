import { ProjectService } from "@/services/project/projectService";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import { useProjectIOStore } from "./projectIOStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

interface ReconcilePathRequest {
  readonly identity: ProjectIdentitySnapshot;
  readonly promise: Promise<string | null>;
}

let reconcilePathInFlight: ReconcilePathRequest | null = null;

function isSameProjectIdentity(
  left: ProjectIdentitySnapshot,
  right: ProjectIdentitySnapshot,
): boolean {
  return left.projectInstanceId === right.projectInstanceId && left.epoch === right.epoch;
}

/**
 * Align `currentPath` with Rust `ProjectState.project_path` when the frontend
 * projection is missing (e.g. HMR store reset while the backend session survives).
 */
export async function reconcileProjectPath(): Promise<string | null> {
  const cached = useProjectIOStore.getState().currentPath;
  if (cached) return cached;

  const identity = captureProjectIdentity();
  if (reconcilePathInFlight && isSameProjectIdentity(reconcilePathInFlight.identity, identity)) {
    return reconcilePathInFlight.promise;
  }

  const promise = (async () => {
    const path = await ProjectService.getProjectPath(identity.projectInstanceId);
    if (!isCurrentProjectIdentity(identity)) return null;
    if (path) {
      useProjectIOStore.getState().setCurrentPath(path);
      return formatDisplayPath(path);
    }
    return null;
  })();
  const request: ReconcilePathRequest = { identity, promise };
  reconcilePathInFlight = request;

  try {
    return await promise;
  } finally {
    if (reconcilePathInFlight === request) {
      reconcilePathInFlight = null;
    }
  }
}

/** Resolve the active project path; reconciles from backend when needed. */
export async function resolveActiveProjectPath(): Promise<string | null> {
  return reconcileProjectPath();
}

/** Subscribe to the active project path projection (reconcile via explicit hydration / save). */
export function useActiveProjectPath(): string | null {
  return useProjectIOStore((state) => state.currentPath);
}
