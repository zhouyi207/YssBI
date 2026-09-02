import { ProjectService } from "@/services/project/projectService";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import { useProjectIOStore } from "./projectIOStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

interface PathHydrationRequest {
  readonly identity: ProjectIdentitySnapshot;
  readonly promise: Promise<string | null>;
}

let pathHydrationInFlight: PathHydrationRequest | null = null;

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
export async function hydrateProjectPath(): Promise<string | null> {
  const cached = useProjectIOStore.getState().currentPath;
  if (cached) return cached;

  const identity = captureProjectIdentity();
  if (pathHydrationInFlight && isSameProjectIdentity(pathHydrationInFlight.identity, identity)) {
    return pathHydrationInFlight.promise;
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
  const request: PathHydrationRequest = { identity, promise };
  pathHydrationInFlight = request;

  try {
    return await promise;
  } finally {
    if (pathHydrationInFlight === request) {
      pathHydrationInFlight = null;
    }
  }
}

/** Resolve the active project path; hydrates from Rust when needed. */
export async function resolveActiveProjectPath(): Promise<string | null> {
  return hydrateProjectPath();
}

/** Subscribe to the active project path projection (refresh through explicit hydration or save). */
export function useActiveProjectPath(): string | null {
  return useProjectIOStore((state) => state.currentPath);
}
