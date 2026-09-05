import { markResourceDirty, markResourceStale } from "@/features/core/resource";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";
import type {
  EditorGraphMutationDto,
  GraphDraftTransformDto,
} from "@/shared/types/domain/editorMutation";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
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

export interface ApplyGraphDraftMutationInput {
  graphPath: string;
  locale: string;
  mutation: EditorGraphMutationDto;
}

export interface GraphDraftCoordinatorDependencies {
  transform(
    projectInstanceId: string,
    graphPath: string,
    locale: string,
    document: NonNullable<ReturnType<typeof getGraphDraftDocument>>,
    mutation: EditorGraphMutationDto,
  ): Promise<GraphDraftTransformDto>;
}

export type ApplyGraphDraftMutationOutcome =
  | { status: "applied"; result: GraphDraftTransformDto; insertedNodeIds: string[] }
  | { status: "noop"; result: GraphDraftTransformDto }
  | { status: "stale"; result?: GraphDraftTransformDto }
  | { status: "saving" }
  | { status: "rejected"; code: GraphDraftRejectionCode };

const defaultDependencies: GraphDraftCoordinatorDependencies = {
  transform: (projectInstanceId, graphPath, locale, document, mutation) =>
    GraphDraftService.transform(projectInstanceId, graphPath, locale, document, mutation),
};

const mutationTails = new Map<string, Promise<void>>();
let coordinatorEpoch = 0;

function installDraftProjection(graphPath: string, result: GraphDraftTransformDto): void {
  const prepared = prepareGraphProjectionReplacements([
    { graphPath, projection: result.projection },
  ]);
  if (!prepared.prepared)
    throw new Error(`Graph draft projection '${graphPath}' could not be installed`);
  useGraphDraftStore.getState().applyTransform(graphPath, result);
  const draft = useGraphDraftStore.getState().sessions[graphPath];
  useHistoryStore.setState({
    canUndo: draft.undoStack.length > 0,
    canRedo: false,
    pending: false,
  });
  commitPreparedGraphProjectionReplacements(prepared.plan);
  const kind = inferGraphResourceKind(graphPath);
  if (kind) {
    markResourceDirty({ id: graphPath, kind }, draft.saveDirty);
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
  const session = useGraphDraftStore.getState().sessions[input.graphPath];
  const isCurrentDraft = () => {
    const current = useGraphDraftStore.getState().sessions[input.graphPath];
    return (
      current?.sessionId === session.sessionId &&
      current.draftGeneration === session.draftGeneration
    );
  };

  let result: GraphDraftTransformDto;
  try {
    result = await dependencies.transform(
      identity.projectInstanceId,
      input.graphPath,
      input.locale,
      document,
      input.mutation,
    );
  } catch (error) {
    if (
      !isCurrentProjectIdentity(identity) ||
      requestEpoch !== coordinatorEpoch ||
      !isCurrentDraft()
    ) {
      return { status: "stale" };
    }
    const code = graphDraftErrorCode(error);
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
    useGraphDraftStore.getState().replaceResolvedProjection(input.graphPath, result.projection);
    commitPreparedGraphProjectionReplacements(prepared.plan);
    return { status: "noop", result };
  }

  const insertedNodeIds = Object.keys(result.document.nodes).filter(
    (nodeId) => !(nodeId in document.nodes),
  );
  installDraftProjection(input.graphPath, result);
  return { status: "applied", result, insertedNodeIds };
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

export function enqueueGraphDraftTask<T>(
  graphPath: string,
  task: () => Promise<T>,
  stale: T,
): Promise<T> {
  const previous = mutationTails.get(graphPath);
  const epoch = coordinatorEpoch;
  const completion = (async () => {
    await previous;
    return epoch === coordinatorEpoch ? task() : stale;
  })();
  const tail = completion.then(
    () => undefined,
    () => undefined,
  );
  mutationTails.set(graphPath, tail);
  void tail.finally(() => {
    if (mutationTails.get(graphPath) === tail) mutationTails.delete(graphPath);
  });
  return completion;
}

export function resetGraphDraftCoordinator(): void {
  coordinatorEpoch += 1;
  mutationTails.clear();
}
