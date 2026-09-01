import type { DeepReadonly } from "@/shared/types/deepReadonly";

export interface ProjectReadSnapshot {
  readonly projectInstanceId: string | null;
  readonly currentPath: string | null;
  readonly status: string;
  readonly error: string | null;
}

export type ReadonlyProjectSnapshot = DeepReadonly<ProjectReadSnapshot>;

export interface ProjectReadCapability {
  readonly getSnapshot: () => ReadonlyProjectSnapshot;
  readonly subscribe: (listener: () => void) => () => void;
}
