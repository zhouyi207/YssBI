import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { markResourceDirty, markResourceStale, useResourceStore } from "@/features/core/resource";
import { getGraphResourceKind } from "@/features/core/resource/resourceSelectors";
import type {
  EditorGraphMutationDto,
  GraphEditResultDto,
  GraphEditVersionDto,
  GraphEditingStateDto,
} from "@/shared/types/domain/editorMutation";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import {
  getGraphDocumentProjection,
  isGraphSaving,
  useGraphEditingStore,
} from "@/features/core/graphEditing";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  GraphEditBusyError,
  graphEditErrorCode,
  type GraphEditRejectionCode,
} from "./graphEditError";

export interface ApplyGraphMutationInput {
  graphPath: string;
  locale?: string;
  mutation:
    | EditorGraphMutationDto
    | ((
        document: NonNullable<ReturnType<typeof getGraphDocumentProjection>>,
      ) => EditorGraphMutationDto);
}

export interface GraphEditCoordinatorDependencies {
  transform(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    version: GraphEditVersionDto,
    mutation: EditorGraphMutationDto,
  ): Promise<GraphEditResultDto>;
}

export type ApplyGraphMutationOutcome =
  | { status: "applied"; result: GraphEditResultDto; insertedNodeIds: string[] }
  | { status: "noop"; result: GraphEditResultDto }
  | { status: "stale"; result?: GraphEditResultDto }
  | { status: "saving" }
  | { status: "rejected"; code: GraphEditRejectionCode };

const defaultDependencies: GraphEditCoordinatorDependencies = {
  transform: (projectInstanceId, graphPath, locale, version, mutation) =>
    GraphEditingService.transform(projectInstanceId, graphPath, locale, version, mutation),
};

const graphTaskQueues = new Map<string, { tail: Promise<void>; pending: number }>();
let coordinatorEpoch = 0;

export function publishGraphEditingState(graphPath: string, editing: GraphEditingStateDto): void {
  const kind = getGraphResourceKind(graphPath);
  if (!kind) return;
  const revision = Number(editing.version.revision);
  if (Number.isSafeInteger(revision))
    useResourceStore.getState().patchResource({ id: graphPath, kind }, { revision });
  markResourceDirty({ id: graphPath, kind }, editing.dirty);
}

export function installGraphEditProjection(graphPath: string, result: GraphEditResultDto): void {
  const prepared = prepareGraphProjectionReplacements([
    { graphPath, projection: result.projection },
  ]);
  if (!prepared.prepared)
    throw new Error(`Graph draft projection '${graphPath}' could not be installed`);
  useGraphEditingStore.getState().applyTransform(graphPath, result);
  const draft = useGraphEditingStore.getState().sessions[graphPath];
  commitPreparedGraphProjectionReplacements(prepared.plan);
  publishGraphEditingState(graphPath, result.editing);
  const kind = getGraphResourceKind(graphPath);
  if (kind) {
    markResourceDirty({ id: graphPath, kind }, draft.saveDirty);
    markResourceStale({ id: graphPath, kind }, false);
  }
}

async function applyDraftMutation(
  input: ApplyGraphMutationInput,
  dependencies: GraphEditCoordinatorDependencies,
  requestEpoch: number,
): Promise<ApplyGraphMutationOutcome> {
  if (requestEpoch !== coordinatorEpoch) return { status: "stale" };
  if (isGraphSaving(input.graphPath)) return { status: "saving" };

  const identity = captureProjectIdentity();
  const document = getGraphDocumentProjection(input.graphPath);
  if (!document) throw new Error(`Graph draft '${input.graphPath}' is not loaded`);
  const session = useGraphEditingStore.getState().sessions[input.graphPath];
  const isCurrentDraft = () => {
    const current = useGraphEditingStore.getState().sessions[input.graphPath];
    return (
      current?.sessionId === session.sessionId &&
      current.projectionGeneration === session.projectionGeneration
    );
  };

  let result: GraphEditResultDto;
  try {
    result = await dependencies.transform(
      identity.projectInstanceId,
      input.graphPath,
      input.locale ?? currentProjectionLocale(),
      session.version,
      typeof input.mutation === "function" ? input.mutation(document) : input.mutation,
    );
  } catch (error) {
    if (
      !isCurrentProjectIdentity(identity) ||
      requestEpoch !== coordinatorEpoch ||
      !isCurrentDraft()
    ) {
      return { status: "stale" };
    }
    const code = graphEditErrorCode(error);
    if (code) return { status: "rejected", code };
    throw error;
  }

  if (
    !isCurrentProjectIdentity(identity) ||
    requestEpoch !== coordinatorEpoch ||
    !isCurrentDraft()
  ) {
    return { status: "stale", result };
  }
  if (!result.changed) {
    const prepared = prepareGraphProjectionReplacements([
      { graphPath: input.graphPath, projection: result.projection },
    ]);
    if (!prepared.prepared) throw new Error("Resolved Graph projection could not be installed");
    useGraphEditingStore.getState().applyTransform(input.graphPath, result);
    commitPreparedGraphProjectionReplacements(prepared.plan);
    return { status: "noop", result };
  }

  const insertedNodeIds = Object.keys(result.document.nodes).filter(
    (nodeId) => !(nodeId in document.nodes),
  );
  installGraphEditProjection(input.graphPath, result);
  return { status: "applied", result, insertedNodeIds };
}

export function applyGraphMutation(
  input: ApplyGraphMutationInput,
  overrides: Partial<GraphEditCoordinatorDependencies> = {},
): Promise<ApplyGraphMutationOutcome> {
  if (isGraphSaving(input.graphPath)) return Promise.resolve({ status: "saving" });
  const dependencies = { ...defaultDependencies, ...overrides };
  const requestEpoch = coordinatorEpoch;
  return enqueueGraphTask<ApplyGraphMutationOutcome>(
    input.graphPath,
    () => applyDraftMutation(input, dependencies, requestEpoch),
    { status: "stale" },
  ).catch((error: unknown) => {
    if (error instanceof GraphEditBusyError) return { status: "rejected", code: error.code };
    throw error;
  });
}

export function enqueueGraphTask<T>(
  graphPath: string,
  task: () => Promise<T>,
  stale: T,
): Promise<T> {
  let queue = graphTaskQueues.get(graphPath);
  if ((queue?.pending ?? 0) >= 64 || (!queue && graphTaskQueues.size >= 128))
    return Promise.reject(new GraphEditBusyError());
  queue ??= { tail: Promise.resolve(), pending: 0 };
  const previous = queue.tail;
  queue.pending++;
  const epoch = coordinatorEpoch;
  const completion = (async () => {
    await previous;
    return epoch === coordinatorEpoch ? task() : stale;
  })();
  const tail = completion.then(
    () => undefined,
    () => undefined,
  );
  queue.tail = tail;
  graphTaskQueues.set(graphPath, queue);
  const owned = queue;
  void tail.finally(() => {
    owned.pending--;
    if (owned.pending === 0 && graphTaskQueues.get(graphPath) === owned)
      graphTaskQueues.delete(graphPath);
  });
  return completion;
}

export function resetGraphEditCoordinator(): void {
  coordinatorEpoch += 1;
  graphTaskQueues.clear();
}
