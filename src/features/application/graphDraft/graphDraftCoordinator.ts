import { markResourceDirty } from "@/features/core/resource";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";
import type {
  EditorGraphMutationDto,
  GraphDraftUpdateDto,
} from "@/shared/types/domain/editorMutation";
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
  ): Promise<GraphDraftUpdateDto>;
}

export type ApplyGraphDraftMutationOutcome =
  | { status: "applied"; result: GraphDraftUpdateDto }
  | { status: "noop"; result: GraphDraftUpdateDto }
  | { status: "stale"; result?: GraphDraftUpdateDto }
  | { status: "saving" }
  | { status: "rejected"; code: GraphDraftRejectionCode };

const defaultDependencies: GraphDraftCoordinatorDependencies = {
  transform: (projectInstanceId, graphPath, locale, document, mutation) =>
    GraphDraftService.transform(projectInstanceId, graphPath, locale, document, mutation),
};

const mutationTails = new Map<string, Promise<void>>();
let coordinatorEpoch = 0;

function installDraftProjection(graphPath: string, result: GraphDraftUpdateDto): void {
  useGraphDraftStore.getState().applyUpdate(graphPath, result);
  const draft = useGraphDraftStore.getState().sessions[graphPath];
  useHistoryStore.setState({
    canUndo: draft.undoStack.length > 0,
    canRedo: false,
    pending: false,
  });
  const currentGeneration =
    useGraphProjectionStore.getState().graphEntities[graphPath]?.requestGeneration ?? 0;
  const applied = useGraphProjectionStore
    .getState()
    .replaceProjection(graphPath, result.projectionReplacement.projection, currentGeneration + 1);
  if (!applied.applied) {
    throw new Error(`Graph draft projection '${graphPath}' could not be installed`);
  }
  const kind = inferGraphResourceKind(graphPath);
  if (kind) markResourceDirty({ id: graphPath, kind }, true);
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

  let result: GraphDraftUpdateDto;
  try {
    result = await dependencies.transform(
      identity.projectInstanceId,
      input.graphPath,
      input.locale,
      document,
      input.mutation,
    );
  } catch (error) {
    if (!isCurrentProjectIdentity(identity) || requestEpoch !== coordinatorEpoch) {
      return { status: "stale" };
    }
    const code = graphDraftErrorCode(error);
    if (code) return { status: "rejected", code };
    throw error;
  }

  if (!isCurrentProjectIdentity(identity) || requestEpoch !== coordinatorEpoch) {
    return { status: "stale", result };
  }
  if (result.projectionReplacement.graphPath !== input.graphPath) {
    throw new Error("Graph draft update targets another graph");
  }
  if (result.patch.operations.length === 0) return { status: "noop", result };
  installDraftProjection(input.graphPath, result);
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
