import { ProjectService } from '@/services/project/projectService';
import { formatDisplayPath } from '@/shared/utils/formatDisplayPath';
import { useProjectIOStore } from './projectIOStore';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

let reconcilePathInFlight: Promise<string | null> | null = null;

/**
 * Align `currentPath` with Rust `ProjectState.project_path` when the frontend
 * projection is missing (e.g. HMR store reset while the backend session survives).
 */
export async function reconcileProjectPath(): Promise<string | null> {
  const cached = useProjectIOStore.getState().currentPath;
  if (cached) return cached;

  if (reconcilePathInFlight) return reconcilePathInFlight;

  const identity = captureProjectIdentity();
  reconcilePathInFlight = (async () => {
    try {
      const path = await ProjectService.getProjectPath(identity.projectInstanceId);
      if (!isCurrentProjectIdentity(identity)) return null;
      if (path) {
        useProjectIOStore.getState().setCurrentPath(path);
        return formatDisplayPath(path);
      }
      return null;
    } finally {
      reconcilePathInFlight = null;
    }
  })();

  return reconcilePathInFlight;
}

/** Resolve the active project path; reconciles from backend when needed. */
export async function resolveActiveProjectPath(): Promise<string | null> {
  return reconcileProjectPath();
}

/** Subscribe to the active project path projection (reconcile via `useProjectSync` / save). */
export function useActiveProjectPath(): string | null {
  return useProjectIOStore((state) => state.currentPath);
}
