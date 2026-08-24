import type { ProjectLifecycleStateSnapshot } from '@/features/core/projectLifecycle/projectLifecycleAuthority';

export interface ProjectIOApplicationPort {
  hydrateFunctionSignatures(graphs: ReadonlyArray<{ path: string; name: string; type: 'event' | 'function' }>): void;
  resetFunctionSignatures(): void;
  resetHistory(): void;
  validatePublicationStart(projectInstanceId: string, revision: number): void;
  startPublication(projectInstanceId: string, revision: number): void;
  acceptProjectActivation(projectInstanceId: string, revision: number): boolean;
  reconcileOpenTabs(): void;
  removeProjectScopedWorkbenchPanels(
    previousProjectInstanceId: string,
    owner: ProjectLifecycleStateSnapshot,
  ): Promise<void>;
  resetGraphProjection(): void;
  beginGraphLoad(graphPath: string): number;
  loadGraphProjection(graphPath: string, token?: number): Promise<boolean>;
  submitPublication(result: unknown): Promise<unknown>;
}

let port: ProjectIOApplicationPort | null = null;

export function registerProjectIOApplicationPort(next: ProjectIOApplicationPort): void {
  port = next;
}

export function resetProjectIOApplicationPort(): void {
  port = null;
}

export function projectIOApplicationPort(): ProjectIOApplicationPort {
  if (!port) throw new Error('Project IO application port is not registered');
  return port;
}
