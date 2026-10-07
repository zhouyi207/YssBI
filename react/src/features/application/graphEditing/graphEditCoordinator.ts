import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { reconcileGraphResultQueries } from "@/features/application/results/runtime";
import type {
  EditorGraphMutationDto,
  GraphEditResultDto,
  GraphEditVersionDto,
  GraphEditorSessionDto,
  GraphConstantDto,
} from "@/shared/types/domain/editorMutation";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import { isGraphSaving } from "@/features/core/graph/read";
import {
  type GraphSessionPublication,
  useResourceStore,
} from "@/features/core/resource/resourceStore";
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
        constants: Readonly<Record<string, GraphConstantDto>> | undefined,
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

export function installGraphSession(
  graphPath: string,
  result: GraphEditorSessionDto,
  options: GraphSessionPublication = { mode: "update" },
): boolean {
  const previous = useResourceStore.getState().sessions[graphPath];
  if (!useResourceStore.getState().installGraphSession(graphPath, result, options)) return false;
  reconcileGraphResultQueries(graphPath, previous);
  return true;
}

async function applyDraftMutation(
  input: ApplyGraphMutationInput,
  dependencies: GraphEditCoordinatorDependencies,
  requestEpoch: number,
): Promise<ApplyGraphMutationOutcome> {
  if (requestEpoch !== coordinatorEpoch) return { status: "stale" };
  // Saving gates admission, not execution of edits already accepted by this FIFO.

  const identity = captureProjectIdentity();
  const state = useResourceStore.getState();
  const session = state.sessions[input.graphPath];
  if (!session) throw new Error(`Graph draft '${input.graphPath}' is not loaded`);
  // Rust projects every document node; the existing entity index owns this membership.
  const nodes = state.graphEntities[input.graphPath].nodes;
  const isCurrentDraft = () => {
    const current = useResourceStore.getState().sessions[input.graphPath];
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
      typeof input.mutation === "function" ? input.mutation(session.constants) : input.mutation,
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
  if (!installGraphSession(input.graphPath, result)) return { status: "stale", result };
  if (!result.changed) return { status: "noop", result };

  const insertedNodeIds = Object.keys(result.document.nodes).filter(
    (nodeId) => !Object.prototype.hasOwnProperty.call(nodes, nodeId),
  );
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
