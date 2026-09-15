import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { markResourceDirty, markResourceStale } from "@/features/core/resource";
import { getGraphResourceKind } from "@/features/core/resource/resourceSelectors";
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
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { graphDraftErrorCode, type GraphDraftRejectionCode } from "./graphDraftError";

export interface ApplyGraphDraftMutationInput {
  graphPath: string;
  locale?: string;
  mutation:
    | EditorGraphMutationDto
    | ((document: NonNullable<ReturnType<typeof getGraphDraftDocument>>) => EditorGraphMutationDto);
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

const graphTaskTails = new Map<string, Promise<void>>();
let coordinatorEpoch = 0;

export function installDraftProjection(graphPath: string, result: GraphDraftTransformDto): void {
  const prepared = prepareGraphProjectionReplacements([
    { graphPath, projection: result.projection },
  ]);
  if (!prepared.prepared)
    throw new Error(`Graph draft projection '${graphPath}' could not be installed`);
  useGraphDraftStore.getState().applyTransform(graphPath, result);
  const draft = useGraphDraftStore.getState().sessions[graphPath];
  commitPreparedGraphProjectionReplacements(prepared.plan);
  const kind = getGraphResourceKind(graphPath);
  if (kind) {
    markResourceDirty({ id: graphPath, kind }, draft.saveDirty);
    markResourceStale({ id: graphPath, kind }, false);
  }
}

async function applyDraftMutation(
  input: ApplyGraphDraftMutationInput,
  dependencies: GraphDraftCoordinatorDependencies,
  requestEpoch: number,
): Promise<ApplyGraphDraftMutationOutcome> {
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
      input.locale ?? currentProjectionLocale(),
      document,
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
  return enqueueGraphDraftTask<ApplyGraphDraftMutationOutcome>(
    input.graphPath,
    () => applyDraftMutation(input, dependencies, requestEpoch),
    { status: "stale" },
  );
}

export function enqueueGraphDraftTask<T>(
  graphPath: string,
  task: () => Promise<T>,
  stale: T,
): Promise<T> {
  const previous = graphTaskTails.get(graphPath);
  const epoch = coordinatorEpoch;
  const completion = (async () => {
    await previous;
    return epoch === coordinatorEpoch ? task() : stale;
  })();
  const tail = completion.then(
    () => undefined,
    () => undefined,
  );
  graphTaskTails.set(graphPath, tail);
  void tail.finally(() => {
    if (graphTaskTails.get(graphPath) === tail) graphTaskTails.delete(graphPath);
  });
  return completion;
}

export function resetGraphDraftCoordinator(): void {
  coordinatorEpoch += 1;
  graphTaskTails.clear();
}
