import { currentProjectionLocale } from "./projectionLocale";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import { markResourceStale } from "@/features/core/resource";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import { getGraphResourceKind } from "@/features/core/resource/resourceSelectors";
import type { GraphEditorSessionDto } from "@/shared/types/domain/editorMutation";
import { getDocumentState } from "@/features/core/resource";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { logger } from "@/features/application/observability/appLogger";
import { resolveCurrentGraphDraft } from "@/features/application/graphDraft/resolveGraphDraft";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

const lifecycleTokenByGraph = new Map<string, number>();
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
    lifecycleTokenByGraph.get(graphPath) !== lifecycleToken
  ) {
    return false;
  }
  const result = useGraphProjectionStore
    .getState()
    .replaceProjection(graphPath, session.projection);
  if (!result.applied) {
    logger.graph.error(
      `Graph projection ${operation} contract invalid for '${graphPath}': ${formatErrorMessage(result.error, "Unknown projection contract error")}`,
      "GraphProjectionLifecycle",
    );
    return false;
  }
  useGraphDraftStore
    .getState()
    [operation === "hydrate" ? "hydrate" : "install"](graphPath, session);
  setGraphProjectionStale(graphPath, false);
  return true;
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
): Promise<GraphEditorSessionDto | false> {
  const identity = { projectInstanceId, epoch: publicationEpoch };
  if (!isCurrentProjectIdentity(identity)) return false;
  const lifecycleToken = startGraphLifecycle(graphPath);
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
}

export function loadGraphProjection(
  graphPath: string,
  lifecycleToken = beginGraphLoadLifecycle(graphPath),
): Promise<boolean> {
  let identity: ProjectIdentitySnapshot;
  try {
    identity = captureProjectIdentity();
  } catch {
    return Promise.resolve(false);
  }
  return requestGraphProjection(
    graphPath,
    "load",
    lifecycleToken,
    identity,
    (path, locale, token) =>
      GraphProjectionService.loadGraph(path, locale, token, identity.projectInstanceId),
  );
}

export function hydrateGraphProjection(graphPath: string, locale: string): Promise<boolean> {
  if (!useGraphProjectionStore.getState().hasGraph(graphPath)) {
    setGraphProjectionStale(graphPath, true);
    return Promise.resolve(false);
  }
  const kind = getGraphResourceKind(graphPath);
  if (kind && getDocumentState({ id: graphPath, kind })?.dirty) {
    setGraphProjectionStale(graphPath, true);
    return resolveCurrentGraphDraft(graphPath, locale)
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
  const identity = captureProjectIdentity();
  const lifecycleToken = startGraphLifecycle(graphPath);
  return requestGraphProjection(
    graphPath,
    "hydrate",
    lifecycleToken,
    identity,
    (path, requestLocale) =>
      GraphProjectionService.hydrateGraph(identity.projectInstanceId, path, requestLocale),
    locale,
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
  lifecycleTokenByGraph.clear();
}
