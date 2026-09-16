import { currentProjectionLocale } from "./projectionLocale";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { isGraphModified, isGraphSaving, useGraphEditingStore } from "@/features/core/graphEditing";
import { markResourceStale } from "@/features/core/resource";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import { clearGraphSyncBaselines } from "@/services/nodeSystem/graphEditorSync";
import { getGraphResourceKind } from "@/features/core/resource/resourceSelectors";
import type { GraphEditorSessionDto } from "@/shared/types/domain/editorMutation";
import type { GraphEditingStateDto } from "@/shared/types/domain/editorMutation";
import { ensureGraphActivity, resetGraphActivity } from "./graphActivity";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { logger } from "@/features/application/observability/appLogger";
import { refreshCurrentGraphProjection } from "@/features/application/graphEditing/refreshGraphProjection";
import {
  enqueueGraphTask,
  installGraphSession,
} from "@/features/application/graphEditing/graphEditCoordinator";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

const lifecycleTokenByGraph = new Map<string, number>();
const pendingActivityRefreshes = new Map<
  string,
  { editing?: GraphEditingStateDto; again: boolean; promise: Promise<boolean> }
>();
let nextLifecycleToken = Date.now() * 1_000;

function startGraphLifecycle(graphPath: string): number {
  const lifecycleToken = ++nextLifecycleToken;
  lifecycleTokenByGraph.set(graphPath, lifecycleToken);
  return lifecycleToken;
}

function setGraphProjectionStale(graphPath: string, stale: boolean): void {
  const kind = getGraphResourceKind(graphPath);
  if (kind) markResourceStale({ id: graphPath, kind }, stale);
}

async function requestGraphProjection(
  graphPath: string,
  operation: "load" | "hydrate",
  lifecycleToken: number,
  identity: ProjectIdentitySnapshot,
  request: (
    graphPath: string,
    locale: string,
    lifecycleToken: number,
  ) => Promise<GraphEditorSessionDto>,
  locale = currentProjectionLocale(),
): Promise<boolean> {
  if (!isCurrentProjectIdentity(identity) || !isGraphLifecycleCurrent(graphPath, lifecycleToken))
    return false;
  setGraphProjectionStale(graphPath, true);
  let session: GraphEditorSessionDto;
  try {
    session = await request(graphPath, locale, lifecycleToken);
  } catch (error) {
    if (!isCurrentProjectIdentity(identity)) return false;
    logger.graph.error(
      `Graph projection ${operation} IPC failed for '${graphPath}': ${formatErrorMessage(error, "Unknown IPC error")}`,
      "GraphProjectionLifecycle",
    );
    return false;
  }

  if (
    !isCurrentProjectIdentity(identity) ||
    lifecycleTokenByGraph.get(graphPath) !== lifecycleToken ||
    isGraphSaving(graphPath)
  ) {
    return false;
  }
  try {
    return installGraphSession(graphPath, session, operation === "load" ? "load" : "update");
  } catch (error) {
    logger.graph.error(
      `Graph projection ${operation} contract invalid for '${graphPath}': ${formatErrorMessage(error)}`,
      "GraphProjectionLifecycle",
    );
    return false;
  }
}

export function beginGraphLoadLifecycle(graphPath: string): number {
  return startGraphLifecycle(graphPath);
}

export function invalidateGraphLifecycle(graphPath: string): number {
  return startGraphLifecycle(graphPath);
}

export function beginGraphUnloadLifecycle(graphPath: string): number {
  return invalidateGraphLifecycle(graphPath);
}

export function beginGraphRenameLifecycle(graphPath: string): number {
  return startGraphLifecycle(graphPath);
}

export function isGraphLifecycleCurrent(graphPath: string, lifecycleToken: number): boolean {
  return lifecycleTokenByGraph.get(graphPath) === lifecycleToken;
}

export async function prepareGraphSessionForPublication(
  graphPath: string,
  projectInstanceId: string,
  publicationEpoch: number,
): Promise<GraphEditorSessionDto | false | null> {
  const identity = { projectInstanceId, epoch: publicationEpoch };
  if (!isCurrentProjectIdentity(identity)) return false;
  const lifecycleToken = startGraphLifecycle(graphPath);
  return enqueueGraphTask<GraphEditorSessionDto | false | null>(
    graphPath,
    async () => {
      if (
        !isCurrentProjectIdentity(identity) ||
        !isGraphLifecycleCurrent(graphPath, lifecycleToken)
      )
        return false;
      // Edits queued before publication may have made this draft dirty since index capture.
      if (isGraphModified(graphPath) || isGraphSaving(graphPath)) return null;
      try {
        const session = await GraphProjectionService.loadGraph(
          graphPath,
          currentProjectionLocale(),
          lifecycleToken,
          projectInstanceId,
        );
        if (
          !isCurrentProjectIdentity(identity) ||
          lifecycleTokenByGraph.get(graphPath) !== lifecycleToken
        ) {
          return false;
        }
        return session;
      } catch (error) {
        if (!isCurrentProjectIdentity(identity)) return false;
        logger.graph.error(
          `Graph projection publication prepare failed for '${graphPath}': ${formatErrorMessage(error, "Unknown IPC error")}`,
          "GraphProjectionLifecycle",
        );
        return false;
      }
    },
    false,
  );
}

export async function loadGraphProjection(
  graphPath: string,
  lifecycleToken = beginGraphLoadLifecycle(graphPath),
): Promise<boolean> {
  let identity: ProjectIdentitySnapshot;
  try {
    identity = captureProjectIdentity();
  } catch {
    return Promise.resolve(false);
  }
  await ensureGraphActivity(identity, refreshGraphFromActivity);
  return enqueueGraphTask(
    graphPath,
    () =>
      requestGraphProjection(graphPath, "load", lifecycleToken, identity, (path, locale, token) =>
        GraphProjectionService.loadGraph(path, locale, token, identity.projectInstanceId),
      ),
    false,
  );
}

export function hydrateGraphProjection(graphPath: string, locale: string): Promise<boolean> {
  const identity = captureProjectIdentity();
  const lifecycleToken = startGraphLifecycle(graphPath);
  return enqueueGraphTask(
    graphPath,
    async () => {
      if (
        !isCurrentProjectIdentity(identity) ||
        !isGraphLifecycleCurrent(graphPath, lifecycleToken)
      )
        return false;
      if (!useGraphProjectionStore.getState().hasGraph(graphPath)) {
        setGraphProjectionStale(graphPath, true);
        return Promise.resolve(false);
      }
      if (isGraphSaving(graphPath)) return false;
      if (isGraphModified(graphPath)) {
        setGraphProjectionStale(graphPath, true);
        return refreshCurrentGraphProjection(graphPath, locale)
          .then((resolved) => {
            if (resolved) setGraphProjectionStale(graphPath, false);
            return resolved;
          })
          .catch((error) => {
            logger.graph.error(
              `Graph draft resolve failed: ${formatErrorMessage(error)}`,
              "GraphProjectionLifecycle",
            );
            return false;
          });
      }
      return requestGraphProjection(
        graphPath,
        "hydrate",
        lifecycleToken,
        identity,
        (path, requestLocale) =>
          GraphProjectionService.hydrateGraph(identity.projectInstanceId, path, requestLocale),
        locale,
      );
    },
    false,
  );
}

export async function hydrateGraphProjections(
  graphPaths: Iterable<string>,
  locale: string,
): Promise<void> {
  await Promise.all(
    [...new Set(graphPaths)].map((graphPath) => hydrateGraphProjection(graphPath, locale)),
  );
}

export function resetGraphProjectionLifecycle(): void {
  clearGraphSyncBaselines();
  resetGraphActivity();
  lifecycleTokenByGraph.clear();
  pendingActivityRefreshes.clear();
}

function refreshGraphFromActivity(
  graphPath: string,
  editing?: GraphEditingStateDto,
): Promise<boolean> {
  const pending = pendingActivityRefreshes.get(graphPath);
  if (pending) {
    pending.editing = editing;
    pending.again = true;
    return pending.promise;
  }
  const identity = captureProjectIdentity();
  const entry = { editing, again: false, promise: Promise.resolve(false) };
  pendingActivityRefreshes.set(graphPath, entry);
  entry.promise = enqueueGraphTask(
    graphPath,
    async () => {
      entry.again = false;
      const editing = entry.editing;
      if (!isCurrentProjectIdentity(identity)) return false;
      const current = useGraphEditingStore.getState().sessions[graphPath];
      if (
        editing &&
        current?.version.sessionId === editing.version.sessionId &&
        BigInt(current.version.revision) > BigInt(editing.version.revision)
      )
        return true;
      if (
        editing &&
        current?.version.sessionId === editing.version.sessionId &&
        current.version.revision === editing.version.revision &&
        current.saveDirty === editing.dirty &&
        current.canUndo === editing.canUndo &&
        current.canRedo === editing.canRedo
      )
        return true;
      const token = startGraphLifecycle(graphPath);
      return requestGraphProjection(graphPath, "hydrate", token, identity, (path, locale) =>
        GraphProjectionService.hydrateGraph(identity.projectInstanceId, path, locale),
      );
    },
    false,
  ).finally(() => {
    if (pendingActivityRefreshes.get(graphPath) !== entry) return;
    pendingActivityRefreshes.delete(graphPath);
    if (entry.again && isCurrentProjectIdentity(identity))
      void refreshGraphFromActivity(graphPath, entry.editing).catch((error: unknown) => {
        logger.graph.error(`Graph refresh failed: ${formatErrorMessage(error)}`, "GraphActivity");
      });
  });
  return entry.promise;
}
