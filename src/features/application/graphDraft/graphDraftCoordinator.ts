import { markResourceDirty, markResourceStale } from "@/features/core/resource";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";
import type {
  EditorGraphMutationDto,
  GraphDraftAcceptedDto,
} from "@/shared/types/domain/editorMutation";
import type { GraphProjectionReplacementDto } from "@/shared/types/domain/editorProjection";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  getGraphDraftDocument,
  isGraphDraftSaving,
  useGraphDraftStore,
} from "@/features/core/graphDraft";
import { useHistoryStore } from "@/features/core/history";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { graphDraftErrorCode, type GraphDraftRejectionCode } from "./graphDraftError";
import {
  invalidateGraphProjectionRequests,
  reserveGraphProjectionRequest,
  type GraphProjectionRequestIdentity,
} from "@/features/application/graphProjection/graphProjectionLifecycle";
import {
  awaitGraphProjection,
  type AwaitedGraphProjection,
} from "@/features/application/graphProjection/graphProjectionCoordinator";

export interface ApplyGraphDraftMutationInput {
  graphPath: string;
  locale: string;
  mutation: EditorGraphMutationDto;
}

export interface GraphDraftCoordinatorDependencies {
  transform(
    projectInstanceId: string,
    graphSessionId: string,
    graphPath: string,
    locale: string,
    acceptedRevision: number,
    requestGeneration: number,
    operationId: string,
    document: NonNullable<ReturnType<typeof getGraphDraftDocument>>,
    mutation: EditorGraphMutationDto,
  ): Promise<GraphDraftAcceptedDto>;
  awaitProjection(
    projectInstanceId: string,
    graphPath: string,
    identity: GraphProjectionRequestIdentity,
  ): AwaitedGraphProjection;
}

export type ApplyGraphDraftMutationOutcome =
  | { status: "applied"; result: GraphDraftAcceptedDto }
  | { status: "noop"; result: GraphDraftAcceptedDto }
  | { status: "stale"; result?: GraphDraftAcceptedDto }
  | { status: "saving" }
  | { status: "rejected"; code: GraphDraftRejectionCode };

const defaultDependencies: GraphDraftCoordinatorDependencies = {
  transform: (
    projectInstanceId,
    graphSessionId,
    graphPath,
    locale,
    acceptedRevision,
    requestGeneration,
    operationId,
    document,
    mutation,
  ) =>
    GraphDraftService.transform(
      projectInstanceId,
      graphSessionId,
      graphPath,
      locale,
      acceptedRevision,
      requestGeneration,
      operationId,
      document,
      mutation,
    ),
  awaitProjection: (projectInstanceId, graphPath, identity) =>
    awaitGraphProjection(projectInstanceId, graphPath, identity),
};

const mutationTails = new Map<string, Promise<void>>();
let coordinatorEpoch = 0;

function installDraftProjection(
  graphPath: string,
  result: GraphDraftAcceptedDto,
  replacement: GraphProjectionReplacementDto,
): void {
  useGraphDraftStore.getState().applyAcceptedUpdate(graphPath, result, replacement.projection);
  const draft = useGraphDraftStore.getState().sessions[graphPath];
  useHistoryStore.setState({
    canUndo: draft.undoStack.length > 0,
    canRedo: false,
    pending: false,
  });
  const current = useGraphProjectionStore.getState().graphEntities[graphPath];
  if (current?.requestGeneration !== result.requestGeneration) {
    const applied = useGraphProjectionStore
      .getState()
      .replaceProjection(graphPath, replacement.projection, result.requestGeneration);
    if (!applied.applied) {
      throw new Error(`Graph draft projection '${graphPath}' could not be installed`);
    }
  }
  const kind = inferGraphResourceKind(graphPath);
  if (kind) {
    markResourceDirty({ id: graphPath, kind }, true);
    markResourceStale({ id: graphPath, kind }, false);
  }
}

async function applyAfterPrevious(
  input: ApplyGraphDraftMutationInput,
  dependencies: GraphDraftCoordinatorDependencies,
  requestEpoch: number,
): Promise<ApplyGraphDraftMutationOutcome> {
  const previous = mutationTails.get(input.graphPath);
  if (previous) await previous;
  if (requestEpoch !== coordinatorEpoch) return { status: "stale" };
  if (isGraphDraftSaving(input.graphPath)) return { status: "saving" };

  const identity = captureProjectIdentity();
  const document = getGraphDraftDocument(input.graphPath);
  if (!document) throw new Error(`Graph draft '${input.graphPath}' is not loaded`);

  const draft = useGraphDraftStore.getState().sessions[input.graphPath];
  if (!draft) throw new Error(`Graph draft '${input.graphPath}' is not loaded`);
  const projectionIdentity = reserveGraphProjectionRequest(input.graphPath);
  const graphKind = inferGraphResourceKind(input.graphPath);
  if (graphKind) markResourceStale({ id: input.graphPath, kind: graphKind }, true);
  const awaited = dependencies.awaitProjection(
    identity.projectInstanceId,
    input.graphPath,
    projectionIdentity,
  );
  void awaited.promise.catch(() => undefined);
  const acceptedRevision = draft.draftRevision + 1;
  const operationId = crypto.randomUUID();

  let result: GraphDraftAcceptedDto;
  try {
    result = await dependencies.transform(
      identity.projectInstanceId,
      projectionIdentity.graphSessionId,
      input.graphPath,
      input.locale,
      acceptedRevision,
      projectionIdentity.requestGeneration,
      operationId,
      document,
      input.mutation,
    );
  } catch (error) {
    invalidateGraphProjectionRequests(input.graphPath);
    awaited.cancel();
    if (graphKind) markResourceStale({ id: input.graphPath, kind: graphKind }, false);
    if (!isCurrentProjectIdentity(identity) || requestEpoch !== coordinatorEpoch) {
      return { status: "stale" };
    }
    const code = graphDraftErrorCode(error);
    if (code) return { status: "rejected", code };
    throw error;
  }

  if (!isCurrentProjectIdentity(identity) || requestEpoch !== coordinatorEpoch) {
    invalidateGraphProjectionRequests(input.graphPath);
    awaited.cancel();
    return { status: "stale", result };
  }
  if (
    result.projectInstanceId !== identity.projectInstanceId ||
    result.graphSessionId !== projectionIdentity.graphSessionId ||
    result.graphPath !== input.graphPath ||
    result.acceptedRevision !== acceptedRevision ||
    result.requestGeneration !== projectionIdentity.requestGeneration ||
    result.operationId !== operationId
  ) {
    invalidateGraphProjectionRequests(input.graphPath);
    awaited.cancel();
    if (graphKind) markResourceStale({ id: input.graphPath, kind: graphKind }, false);
    throw new Error("Graph draft acceptance identity is inconsistent");
  }
  if (result.patch.operations.length === 0) {
    awaited.cancel();
    if (graphKind) markResourceStale({ id: input.graphPath, kind: graphKind }, false);
    useGraphDraftStore.getState().acceptNoop(input.graphPath, result.acceptedRevision);
    return { status: "noop", result };
  }
  let replacement: GraphProjectionReplacementDto;
  try {
    replacement = await awaited.promise;
  } catch (error) {
    invalidateGraphProjectionRequests(input.graphPath);
    if (graphKind) markResourceStale({ id: input.graphPath, kind: graphKind }, false);
    throw error;
  }
  if (!isCurrentProjectIdentity(identity) || requestEpoch !== coordinatorEpoch) {
    invalidateGraphProjectionRequests(input.graphPath);
    return { status: "stale", result };
  }
  installDraftProjection(input.graphPath, result, replacement);
  return { status: "applied", result };
}

export function applyGraphDraftMutation(
  input: ApplyGraphDraftMutationInput,
  overrides: Partial<GraphDraftCoordinatorDependencies> = {},
): Promise<ApplyGraphDraftMutationOutcome> {
  if (isGraphDraftSaving(input.graphPath)) return Promise.resolve({ status: "saving" });
  const dependencies = { ...defaultDependencies, ...overrides };
  const requestEpoch = coordinatorEpoch;
  const completion = applyAfterPrevious(input, dependencies, requestEpoch);
  const tail = completion.then(
    () => undefined,
    () => undefined,
  );
  mutationTails.set(input.graphPath, tail);
  void tail.finally(() => {
    if (mutationTails.get(input.graphPath) === tail) mutationTails.delete(input.graphPath);
  });
  return completion;
}

export async function waitForGraphDraftMutations(graphPath: string): Promise<void> {
  await mutationTails.get(graphPath);
}

export function resetGraphDraftCoordinator(): void {
  coordinatorEpoch += 1;
  mutationTails.clear();
}
