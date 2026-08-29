import type { ErrorReference } from '@/features/application/errorReference';

type Awaitable<T> = T | PromiseLike<T>;

export interface ProjectHydrationIdentity {
  readonly projectInstanceId: string;
  readonly epoch: number;
}

export type ProjectHydrationOutcome =
  | { readonly status: 'published' }
  | { readonly status: 'stale' }
  | { readonly status: 'notReady' }
  | { readonly status: 'cancelled' }
  | { readonly status: 'failed' };

export interface ProjectHydrationCoordinator {
  loadCurrentProject(): Promise<ProjectHydrationOutcome>;
  refreshResourceIndex(): Promise<ProjectHydrationOutcome>;
  loadGraph(graphPath: string): Promise<ProjectHydrationOutcome>;
  replaceProject(): void;
}

export interface ProjectHydrationDependencies<
  TProjectSnapshot,
  TResourceSnapshot = never,
  TGraphSnapshot = never,
> {
  captureProjectIdentity(): ProjectHydrationIdentity | null;
  loadProjectIndex(
    identity: ProjectHydrationIdentity,
  ): PromiseLike<TProjectSnapshot> | Promise<TProjectSnapshot>;
  publishProjectSnapshot(
    snapshot: TProjectSnapshot,
    identity: ProjectHydrationIdentity,
  ): Awaitable<void>;
  loadResourceIndex?(
    identity: ProjectHydrationIdentity,
  ): PromiseLike<TResourceSnapshot> | Promise<TResourceSnapshot>;
  publishResourceIndex?(
    snapshot: TResourceSnapshot,
    identity: ProjectHydrationIdentity,
  ): Awaitable<void>;
  loadGraph?(
    graphPath: string,
    identity: ProjectHydrationIdentity,
  ): PromiseLike<TGraphSnapshot> | Promise<TGraphSnapshot>;
  publishGraph?(
    graphPath: string,
    snapshot: TGraphSnapshot,
    identity: ProjectHydrationIdentity,
  ): Awaitable<void>;
  publishFailure?(
    error: ErrorReference,
    operation: 'loadCurrentProject' | 'refreshResourceIndex' | 'loadGraph',
    identity: ProjectHydrationIdentity,
  ): Awaitable<void>;
  toErrorReference?(error: unknown, operation: string): ErrorReference;
}

interface RequestOwner {
  readonly identity: ProjectHydrationIdentity;
  readonly coordinatorEpoch: number;
  readonly generation: number;
  readonly operationKey: string;
}

function fallbackErrorReference(operation: string): ErrorReference {
  const code = operation === 'loadCurrentProject'
    ? 'project_load_failed'
    : operation === 'refreshResourceIndex'
      ? 'project_resource_index_failed'
      : 'project_graph_load_failed';
  return {
    code,
    incidentId: null,
  };
}

export function createProjectHydrationCoordinator<
  TProjectSnapshot,
  TResourceSnapshot = never,
  TGraphSnapshot = never,
>(
  dependencies: ProjectHydrationDependencies<
    TProjectSnapshot,
    TResourceSnapshot,
    TGraphSnapshot
  >,
): ProjectHydrationCoordinator {
  let coordinatorEpoch = 0;
  let nextGeneration = 0;
  const pending = new Map<string, Promise<ProjectHydrationOutcome>>();
  const latestGeneration = new Map<string, number>();

  const captureIdentity = (): ProjectHydrationIdentity | null => {
    try {
      return dependencies.captureProjectIdentity();
    } catch {
      return null;
    }
  };

  const isCurrent = (owner: RequestOwner): boolean => {
    if (owner.coordinatorEpoch !== coordinatorEpoch) return false;
    if (latestGeneration.get(owner.operationKey) !== owner.generation) return false;
    const current = captureIdentity();
    return current?.projectInstanceId === owner.identity.projectInstanceId
      && current.epoch === owner.identity.epoch;
  };

  const safeErrorReference = (error: unknown, operation: string): ErrorReference => {
    try {
      return dependencies.toErrorReference?.(error, operation)
        ?? fallbackErrorReference(operation);
    } catch {
      return fallbackErrorReference(operation);
    }
  };

  const startRequest = (
    operationKey: string,
    owner: RequestOwner,
    operation: 'loadCurrentProject' | 'refreshResourceIndex' | 'loadGraph',
    work: () => Promise<unknown> | PromiseLike<unknown>,
    publish: (value: unknown, identity: ProjectHydrationIdentity) => Awaitable<void>,
  ): Promise<ProjectHydrationOutcome> => {
    const existing = pending.get(operationKey);
    if (existing) return existing;

    let request!: Promise<ProjectHydrationOutcome>;
    request = (async (): Promise<ProjectHydrationOutcome> => {
      try {
        const snapshot = await work();
        if (!isCurrent(owner)) return { status: 'stale' };
        await publish(snapshot, owner.identity);
        if (!isCurrent(owner)) return { status: 'stale' };
        return { status: 'published' };
      } catch (error) {
        if (!isCurrent(owner)) return { status: 'stale' };
        try {
          await dependencies.publishFailure?.(
            safeErrorReference(error, operation),
            operation,
            owner.identity,
          );
        } catch {
          // Failure publication is advisory and must not leak an untyped rejection.
        }
        return { status: 'failed' };
      } finally {
        if (pending.get(operationKey) === request) pending.delete(operationKey);
        if (latestGeneration.get(operationKey) === owner.generation) {
          latestGeneration.delete(operationKey);
        }
      }
    })();

    pending.set(operationKey, request);
    return request;
  };

  const createOwner = (
    operation: string,
    identity: ProjectHydrationIdentity,
    suffix = '',
  ): RequestOwner => {
    const operationKey = `${coordinatorEpoch}:${operation}:${identity.projectInstanceId}:${identity.epoch}:${suffix}`;
    const generation = ++nextGeneration;
    latestGeneration.set(operationKey, generation);
    return {
      identity,
      coordinatorEpoch,
      generation,
      operationKey,
    };
  };

  const operationKeyFor = (
    operation: string,
    identity: ProjectHydrationIdentity,
    suffix = '',
  ): string => `${coordinatorEpoch}:${operation}:${identity.projectInstanceId}:${identity.epoch}:${suffix}`;

  const pendingRequest = (
    operation: string,
    identity: ProjectHydrationIdentity,
    suffix = '',
  ): Promise<ProjectHydrationOutcome> | undefined => pending.get(
    operationKeyFor(operation, identity, suffix),
  );

  const loadCurrentProject = (): Promise<ProjectHydrationOutcome> => {
    const identity = captureIdentity();
    if (!identity) return Promise.resolve({ status: 'notReady' });
    const existing = pendingRequest('load', identity);
    if (existing) return existing;
    const owner = createOwner('load', identity);
    return startRequest(
      owner.operationKey,
      owner,
      'loadCurrentProject',
      () => dependencies.loadProjectIndex(identity),
      (snapshot, currentIdentity) => dependencies.publishProjectSnapshot(
        snapshot as TProjectSnapshot,
        currentIdentity,
      ),
    );
  };

  const refreshResourceIndex = (): Promise<ProjectHydrationOutcome> => {
    const identity = captureIdentity();
    if (!identity || !dependencies.loadResourceIndex || !dependencies.publishResourceIndex) {
      return Promise.resolve({ status: 'notReady' });
    }
    const existing = pendingRequest('resource-index', identity);
    if (existing) return existing;
    const owner = createOwner('resource-index', identity);
    return startRequest(
      owner.operationKey,
      owner,
      'refreshResourceIndex',
      () => dependencies.loadResourceIndex!(identity),
      (snapshot, currentIdentity) => dependencies.publishResourceIndex!(
        snapshot as TResourceSnapshot,
        currentIdentity,
      ),
    );
  };

  const loadGraph = (graphPath: string): Promise<ProjectHydrationOutcome> => {
    const identity = captureIdentity();
    if (!identity || !dependencies.loadGraph || !dependencies.publishGraph) {
      return Promise.resolve({ status: 'notReady' });
    }
    const existing = pendingRequest('graph', identity, graphPath);
    if (existing) return existing;
    const owner = createOwner('graph', identity, graphPath);
    return startRequest(
      owner.operationKey,
      owner,
      'loadGraph',
      () => dependencies.loadGraph!(graphPath, identity),
      (snapshot, currentIdentity) => dependencies.publishGraph!(
        graphPath,
        snapshot as TGraphSnapshot,
        currentIdentity,
      ),
    );
  };

  return {
    loadCurrentProject,
    refreshResourceIndex,
    loadGraph,
    replaceProject: () => {
      coordinatorEpoch += 1;
      nextGeneration += 1;
    },
  };
}
