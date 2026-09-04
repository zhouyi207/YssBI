import { markResourceStale } from "@/features/core/resource";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  isGraphProjectionRequestCurrent,
  type GraphProjectionRequestIdentity,
} from "./graphProjectionLifecycle";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";
import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { GraphProjectionReplacementDto } from "@/shared/types/domain/editorProjection";
import type {
  GraphProjectionChannelEventDto,
  GraphProjectionPublicationDto,
  GraphProjectionSnapshotDto,
} from "@/shared/types/domain/graphProjectionChannel";
import { recoverGraphProjectionSnapshot } from "./graphProjectionRecovery";
import { requestGraphProjectionReconnect } from "./graphProjectionConnection";

const PROJECTION_WAIT_TIMEOUT_MS = 5_000;

interface PendingProjection {
  readonly projectInstanceId: string;
  readonly graphSessionId: string;
  readonly graphPath: string;
  readonly requestGeneration: number;
  readonly resolve: (replacement: GraphProjectionReplacementDto) => void;
  readonly reject: (error: Error) => void;
  readonly timeout: ReturnType<typeof setTimeout>;
}

export interface AwaitedGraphProjection {
  readonly promise: Promise<GraphProjectionReplacementDto>;
  cancel(): void;
}

const pendingByGraph = new Map<string, PendingProjection>();
let coordinatorEpoch = 0;

function requestKey(projectInstanceId: string, graphSessionId: string, graphPath: string): string {
  return `${projectInstanceId}\u0000${graphSessionId}\u0000${graphPath}`;
}

function setProjectionStale(graphPath: string, stale: boolean): void {
  const kind = inferGraphResourceKind(graphPath);
  if (kind) markResourceStale({ id: graphPath, kind }, stale);
}

function isCurrentProject(projectInstanceId: string): boolean {
  return captureProjectLifecycleState().projectInstanceId === projectInstanceId;
}

function rejectPending(
  publication: {
    projectInstanceId: string;
    graphSessionId: string;
    graphPath: string;
    requestGeneration: number;
  },
  message: string,
): void {
  const key = requestKey(
    publication.projectInstanceId,
    publication.graphSessionId,
    publication.graphPath,
  );
  const pending = pendingByGraph.get(key);
  if (!pending || pending.requestGeneration !== publication.requestGeneration) return;
  clearTimeout(pending.timeout);
  pendingByGraph.delete(key);
  pending.reject(new Error(message));
}

function isPublicationCurrent(publication: GraphProjectionPublicationDto): boolean {
  return (
    isCurrentProject(publication.projectInstanceId) &&
    isGraphProjectionRequestCurrent(
      publication.graphPath,
      publication.graphSessionId,
      publication.requestGeneration,
    ) &&
    publication.replacement.graphPath === publication.graphPath
  );
}

function resolvePendingPublication(publication: GraphProjectionPublicationDto): boolean {
  const key = requestKey(
    publication.projectInstanceId,
    publication.graphSessionId,
    publication.graphPath,
  );
  const pending = pendingByGraph.get(key);
  if (pending?.requestGeneration === publication.requestGeneration) {
    clearTimeout(pending.timeout);
    pendingByGraph.delete(key);
    pending.resolve(publication.replacement);
    return true;
  }
  return false;
}

function hasPendingPublication(publication: GraphProjectionPublicationDto): boolean {
  const pending = pendingByGraph.get(
    requestKey(publication.projectInstanceId, publication.graphSessionId, publication.graphPath),
  );
  return pending?.requestGeneration === publication.requestGeneration;
}

function acceptPublication(publication: GraphProjectionPublicationDto): boolean {
  if (!isPublicationCurrent(publication)) return false;
  if (resolvePendingPublication(publication)) return true;

  const result = useGraphProjectionStore
    .getState()
    .replaceProjection(
      publication.graphPath,
      publication.replacement.projection,
      publication.requestGeneration,
    );
  const current = useGraphProjectionStore.getState().graphEntities[publication.graphPath];
  if (
    !result.applied &&
    !(
      result.reason === "stale-generation" &&
      current?.requestGeneration === publication.requestGeneration
    )
  ) {
    rejectPending(publication, `Graph Projection '${publication.graphPath}' was rejected`);
    return false;
  }

  setProjectionStale(publication.graphPath, false);
  return true;
}

export function acceptGraphProjectionSnapshot(snapshot: GraphProjectionSnapshotDto): void {
  if (!isCurrentProject(snapshot.projectInstanceId)) return;
  const publications = snapshot.projections.filter(
    (publication) =>
      publication.projectInstanceId === snapshot.projectInstanceId &&
      snapshot.latestGenerationByGraph[publication.graphPath] === publication.requestGeneration &&
      isPublicationCurrent(publication),
  );
  const staged = publications.filter(hasPendingPublication);
  const direct = publications.filter((publication) => !hasPendingPublication(publication));
  const replacements = direct.filter((publication) => {
    const current = useGraphProjectionStore.getState().graphEntities[publication.graphPath];
    return !current || current.requestGeneration < publication.requestGeneration;
  });
  if (replacements.length > 0) {
    const result = useGraphProjectionStore.getState().replacePublishedProjectionsAtomically(
      replacements.map((publication) => ({
        ...publication.replacement,
        requestGeneration: publication.requestGeneration,
      })),
    );
    if (!result.applied) {
      replacements.forEach((publication) => setProjectionStale(publication.graphPath, true));
      return;
    }
  }
  direct.forEach((publication) => setProjectionStale(publication.graphPath, false));
  staged.forEach(resolvePendingPublication);
}

export function acceptGraphProjectionEvent(event: GraphProjectionChannelEventDto): void {
  if (!isCurrentProject(event.projectInstanceId)) return;
  if (event.type === "projectionReplaced") {
    acceptPublication(event);
    return;
  }
  if (event.type === "projectionBatchReplaced") {
    const publications = event.replacements.filter(isPublicationCurrent);
    const staged = publications.filter(hasPendingPublication);
    const direct = publications.filter((publication) => !hasPendingPublication(publication));
    if (direct.length > 0) {
      const result = useGraphProjectionStore.getState().replacePublishedProjectionsAtomically(
        direct.map((publication) => ({
          ...publication.replacement,
          requestGeneration: publication.requestGeneration,
        })),
      );
      if (result.applied) {
        direct.forEach((publication) => {
          setProjectionStale(publication.graphPath, false);
        });
      } else {
        direct.forEach((publication) => {
          setProjectionStale(publication.graphPath, true);
          rejectPending(publication, `Graph Projection batch was rejected: ${result.reason}`);
        });
      }
    }
    staged.forEach(resolvePendingPublication);
    if (event.status.status === "incomplete") {
      for (const graphPath of event.status.invalidatedGraphPaths) {
        setProjectionStale(graphPath, true);
      }
    }
    return;
  }
  if (
    !isGraphProjectionRequestCurrent(event.graphPath, event.graphSessionId, event.requestGeneration)
  ) {
    return;
  }
  setProjectionStale(event.graphPath, true);
  rejectPending(
    event,
    `Graph Projection resolution failed: ${event.reasonCode}${event.incidentId ? ` (${event.incidentId})` : ""}`,
  );
}

export function awaitGraphProjection(
  projectInstanceId: string,
  graphPath: string,
  identity: GraphProjectionRequestIdentity,
): AwaitedGraphProjection {
  const epoch = coordinatorEpoch;
  const key = requestKey(projectInstanceId, identity.graphSessionId, graphPath);
  const previous = pendingByGraph.get(key);
  if (previous) {
    clearTimeout(previous.timeout);
    previous.reject(new Error("Graph Projection request was superseded"));
    pendingByGraph.delete(key);
  }

  let cancel = () => undefined;
  const promise = new Promise<GraphProjectionReplacementDto>((resolve, reject) => {
    const timeout = setTimeout(() => {
      if (coordinatorEpoch !== epoch) return;
      void recoverGraphProjectionSnapshot(projectInstanceId)
        .then((snapshot) => {
          if (coordinatorEpoch !== epoch) return;
          acceptGraphProjectionSnapshot(snapshot);
          requestGraphProjectionReconnect();
          const pending = pendingByGraph.get(key);
          if (pending?.requestGeneration !== identity.requestGeneration) return;
          pendingByGraph.delete(key);
          pending.reject(new Error("Graph Projection publication timed out"));
        })
        .catch((error) => {
          const pending = pendingByGraph.get(key);
          if (pending?.requestGeneration !== identity.requestGeneration) return;
          pendingByGraph.delete(key);
          pending.reject(
            error instanceof Error ? error : new Error("Graph Projection snapshot recovery failed"),
          );
          requestGraphProjectionReconnect();
        });
    }, PROJECTION_WAIT_TIMEOUT_MS);
    const pending: PendingProjection = {
      projectInstanceId,
      graphSessionId: identity.graphSessionId,
      graphPath,
      requestGeneration: identity.requestGeneration,
      resolve,
      reject,
      timeout,
    };
    pendingByGraph.set(key, pending);
    cancel = () => {
      if (pendingByGraph.get(key) !== pending) return;
      clearTimeout(timeout);
      pendingByGraph.delete(key);
      reject(new Error("Graph Projection request was cancelled"));
    };
  });
  return { promise, cancel };
}

export async function recoverCurrentGraphProjections(projectInstanceId: string): Promise<void> {
  acceptGraphProjectionSnapshot(await recoverGraphProjectionSnapshot(projectInstanceId));
}

export function cancelGraphProjectionRequests(graphPath: string): void {
  for (const [key, pending] of pendingByGraph) {
    if (pending.graphPath !== graphPath) continue;
    clearTimeout(pending.timeout);
    pendingByGraph.delete(key);
    pending.reject(new Error("Graph Projection session was invalidated"));
  }
}

export function resetGraphProjectionCoordinator(): void {
  coordinatorEpoch += 1;
  for (const pending of pendingByGraph.values()) {
    clearTimeout(pending.timeout);
    pending.reject(new Error("Graph Projection coordinator was reset"));
  }
  pendingByGraph.clear();
}
