import { produce } from "immer";
import { prepareSnapshotResources } from "./projectSnapshotResources";
import { nodeFileEntries } from "@/shared/types/domain/project";
import {
  detailResource,
  detailResourceRef,
} from "@/features/core/editor/detail/editorDetailPolicy";
import { remapDocumentInputs } from "@/features/application/resource/documentInputs";
import { shouldRetainResourceEditor } from "@/features/core/resource";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import type { ProjectIndexRow } from "@/shared/types/domain/project";
import { parseProjectIndexRow } from "@/services/project/projectService";
import { prepareDatabaseIndexSnapshot } from "@/features/application/dataManagement/databaseRecords";
import type {
  PreparedProjectSnapshot,
  ProjectSnapshotPreparation,
} from "./projectPublicationCoordinator";
import { prepareGraphMetaSnapshot } from "@/features/core/dataStore/graphMeta";
import {
  prepareGraphSessions,
  canAcceptGraphSession,
} from "@/features/core/dataStore/graphProjection";
import { isGraphModified, isGraphSaving } from "@/features/core/graph/read";

import {
  assertCurrentProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { resourceKey, useResourceStore, type ResourceKey } from "@/features/core/resource";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { useViewportStore } from "@/features/core/viewport";
import { parseViewportScopeKey, viewportScopeKey } from "@/features/core/viewport/viewportScope";
import {
  remapGraphNonViewportUiState,
  remapFileNonViewportUiState,
} from "@/features/application/editor/cascadeGraphPathReferences";
import { invalidateChartPreviewCacheForMove } from "@/services/chart/chartPreviewCache";
import { commitEditorLayoutPublication } from "./editorLayoutPublicationCommit";
import { reconcileGraphResultQueries } from "@/features/application/results/runtime";
import { releaseDocumentInputs } from "@/features/application/resource/documentInputs";

export function validateProjectSnapshotIndex(
  index: ProjectIndexRow,
  projectInstanceId: string,
): string | undefined {
  if (index.projectInstanceId !== projectInstanceId) return "snapshot project identity is stale";
  try {
    parseProjectIndexRow(index);
  } catch {
    return "project index contract is malformed";
  }
  return undefined;
}

function canReach(
  movesBySource: ReadonlyMap<string, ReadonlySet<string>>,
  from: string,
  destination: string,
): boolean {
  const pending = [from];
  const visited = new Set<string>();
  while (pending.length > 0) {
    const path = pending.pop() as string;
    if (path === destination) return true;
    if (visited.has(path)) continue;
    visited.add(path);
    pending.push(...(movesBySource.get(path) ?? []));
  }
  return false;
}

function authoritativeTerminals(
  authoritativeGraphPaths: ReadonlySet<string>,
  movesBySource: ReadonlyMap<string, ReadonlySet<string>>,
  source: string,
): ReadonlySet<string> {
  const terminals = new Set<string>();
  const pending = [...(movesBySource.get(source) ?? [])];
  const visited = new Set<string>([source]);
  while (pending.length > 0) {
    const path = pending.pop() as string;
    if (authoritativeGraphPaths.has(path)) {
      terminals.add(path);
      continue;
    }
    if (visited.has(path)) continue;
    visited.add(path);
    pending.push(...(movesBySource.get(path) ?? []));
  }
  return terminals;
}

function buildSnapshotPathRemaps(
  authoritativePaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
  accepts: (move: ResourceMutationResultDto["moves"][number]) => boolean,
): ReadonlyMap<string, string> {
  const movesBySource = new Map<string, Set<string>>();
  for (const result of queuedResults) {
    for (const move of result.moves) {
      if (!accepts(move)) continue;
      const destinations = movesBySource.get(move.from) ?? new Set<string>();
      destinations.add(move.to);
      movesBySource.set(move.from, destinations);
    }
  }

  const destinationOwners = new Map<string, string>();
  const pathRemaps = new Map<string, string>();
  for (const source of movesBySource.keys()) {
    if (authoritativePaths.has(source)) continue;
    const terminals = [...authoritativeTerminals(authoritativePaths, movesBySource, source)];
    if (terminals.length > 1) {
      throw new Error(`conflicting recovery move source '${source}'`);
    }
    const terminal = terminals[0];
    if (!terminal) continue;
    const destinationOwner = destinationOwners.get(terminal);
    if (
      destinationOwner &&
      !canReach(movesBySource, destinationOwner, source) &&
      !canReach(movesBySource, source, destinationOwner)
    ) {
      throw new Error(`conflicting recovery move destination '${terminal}'`);
    }
    pathRemaps.set(source, terminal);
    destinationOwners.set(terminal, destinationOwner ?? source);
  }
  return pathRemaps;
}

export function buildProjectSnapshotPathRemaps(
  authoritativeGraphPaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  return buildSnapshotPathRemaps(
    authoritativeGraphPaths,
    queuedResults,
    (move) => move.kind === "event_graph" || move.kind === "function_graph",
  );
}

export function buildProjectSnapshotFilePathRemaps(
  authoritativeFilePaths: ReadonlySet<string>,
  queuedResults: readonly ResourceMutationResultDto[],
): ReadonlyMap<string, string> {
  return buildSnapshotPathRemaps(
    authoritativeFilePaths,
    queuedResults,
    (move) => move.kind === "chart" || move.kind === "mind" || move.kind === "doc",
  );
}

function prepareViewports(
  current: ReturnType<typeof useViewportStore.getState>["viewports"],
  pathRemaps: ReadonlyMap<string, string>,
  authoritativeGraphPaths: ReadonlySet<string>,
) {
  return produce(current, (viewports) => {
    for (const [from, to] of pathRemaps) {
      for (const key of Object.keys(viewports)) {
        const scope = parseViewportScopeKey(key);
        if (!scope || scope.graphPath !== from) continue;
        const destinationKey = viewportScopeKey({ ...scope, graphPath: to });
        if (viewports[destinationKey]) {
          throw new Error(`recovery viewport destination '${destinationKey}' already exists`);
        }
        viewports[destinationKey] = viewports[key];
        delete viewports[key];
      }
    }
    for (const key of Object.keys(viewports)) {
      const scope = parseViewportScopeKey(key);
      if (scope && !authoritativeGraphPaths.has(scope.graphPath)) delete viewports[key];
    }
  });
}

export function prepareProjectSnapshotCommit(
  plan: ProjectSnapshotPreparation,
): PreparedProjectSnapshot {
  const current = useResourceStore.getState();
  const databases = prepareDatabaseIndexSnapshot(
    plan.index.databases,
    current.databases,
    current.resources,
    undefined,
    plan.databaseMetadata,
  );
  const graphs = nodeFileEntries(plan.index);
  const graphMeta = prepareGraphMetaSnapshot(graphs, useResourceStore.getState().graphMeta);
  const graphsByPath = new Map(graphs.map((graph) => [graph.path, graph]));
  const authoritativeGraphPaths = new Set(graphsByPath.keys());
  const replacements = [...plan.graphSessions]
    .filter(([path, session]) => {
      const previousPath = [...plan.pathRemaps].find(([, to]) => to === path)?.[0] ?? path;
      return (
        !isGraphModified(previousPath) &&
        !isGraphSaving(previousPath) &&
        canAcceptGraphSession(useResourceStore.getState().sessions[previousPath], session) &&
        canAcceptGraphSession(useResourceStore.getState().sessions[path], session)
      );
    })
    .map(([graphPath, session]) => ({ graphPath, session }));
  const retainedGraphPaths = new Set([
    ...authoritativeGraphPaths,
    ...Object.keys(useResourceStore.getState().sessions).filter(
      (path) =>
        !plan.deletedResources.has(resourceKey({ id: path, kind: "event_graph" })) &&
        !plan.deletedResources.has(resourceKey({ id: path, kind: "function_graph" })) &&
        (isGraphModified(path) || isGraphSaving(path)),
    ),
  ]);
  const preparedGraphs = prepareGraphSessions(
    replacements,
    retainedGraphPaths,
    useResourceStore.getState(),
  );
  const refreshedKeys = [
    ...replacements.map(({ graphPath }) => {
      const kind = graphsByPath.get(graphPath)!.type;
      return resourceKey({ id: graphPath, kind });
    }),
    ...[...plan.chartDocuments.keys()].map((id) => resourceKey({ id, kind: "chart" })),
  ];
  const { resources, documents, chartDocuments, fileSnapshots } = prepareSnapshotResources(
    plan,
    {
      resources: useResourceStore.getState().resources,
      documents: useResourceStore.getState().documents,
      chartDocuments: useResourceStore.getState().chartDocuments,
      fileSnapshots: useResourceStore.getState().fileSnapshots,
    },
    refreshedKeys,
  );
  const focused = useGraphSessionStore.getState().focusedSession;
  const remappedFocusedPath = focused
    ? (plan.pathRemaps.get(focused.graphPath) ?? focused.graphPath)
    : null;
  const focusedSession =
    focused && remappedFocusedPath && authoritativeGraphPaths.has(remappedFocusedPath)
      ? { ...focused, graphPath: remappedFocusedPath }
      : null;
  const viewports = prepareViewports(
    useViewportStore.getState().viewports,
    plan.pathRemaps,
    authoritativeGraphPaths,
  );

  return {
    ...plan,
    graphProjectionPlan: preparedGraphs,
    storeState: {
      resources,
      graphOrder: graphs.map((graph) => graph.path),
      documents,
      graphMeta,
      databases,
      chartDocuments,
      fileSnapshots,
      focusedSession,
      viewports,
    },
  };
}

export function commitPreparedProjectSnapshot(
  prepared: PreparedProjectSnapshot,
): void | Promise<void> {
  const moves = [
    ...[...prepared.pathRemaps].map(([from, to]) => ({ from, to })),
    ...[...prepared.filePathRemaps].map(([from, to]) => ({ from, to })),
  ];
  return commitEditorLayoutPublication(
    moves,
    prepared.storeState.resources,
    prepared.deletedResources,
    () => {
      assertCurrentProjectIdentity(prepared);
      const plan = prepareProjectSnapshotCommit(prepared);
      const fileProjections = [
        { kind: "mind", index: plan.index.minds },
        { kind: "doc", index: plan.index.docs },
      ] as const;
      for (const { kind, index } of fileProjections) {
        const snapshots = useResourceStore.getState().fileSnapshots[kind];
        for (const [from, to] of plan.filePathRemaps) {
          const entry = index.find((file) => file.path === to);
          const previous = snapshots[from];
          if (entry && previous) {
            remapDocumentInputs(from, to, previous.version, {
              sessionId: previous.version.sessionId,
              revision: entry.revision,
            });
            assertCurrentProjectIdentity(prepared);
          }
        }
        for (const path of Object.keys(snapshots)) {
          if (!plan.storeState.fileSnapshots[kind][path]) {
            releaseDocumentInputs(path);
            assertCurrentProjectIdentity(prepared);
          }
        }
      }
      const previousGraphs = useResourceStore.getState().sessions;
      useResourceStore.getState().setSnapshot({
        databases: plan.storeState.databases,
        resources: Object.values(plan.storeState.resources),
        documents: plan.storeState.documents,
        chartDocuments: plan.storeState.chartDocuments,
        fileSnapshots: plan.storeState.fileSnapshots,
        graphProjection: plan.graphProjectionPlan.state,
        graphMeta: plan.storeState.graphMeta,
        graphOrder: plan.storeState.graphOrder,
        publicationRevision: plan.publicationRevision,
      });
      assertCurrentProjectIdentity(prepared);
      useSidebarStore.getState().publishPanels(plan.activityPanels);
      assertCurrentProjectIdentity(prepared);
      for (const path of new Set([
        ...plan.graphProjectionPlan.graphPaths,
        ...Object.keys(previousGraphs).filter(
          (path) => !plan.graphProjectionPlan.state.sessions[path],
        ),
      ])) {
        reconcileGraphResultQueries(path, previousGraphs[path]);
        assertCurrentProjectIdentity(prepared);
      }
      useGraphSessionStore.setState({ focusedSession: plan.storeState.focusedSession });
      assertCurrentProjectIdentity(prepared);
      useViewportStore.setState({ viewports: plan.storeState.viewports });
      assertCurrentProjectIdentity(prepared);
      for (const [from, to] of plan.pathRemaps) {
        remapGraphNonViewportUiState(from, to, prepared);
        assertCurrentProjectIdentity(prepared);
      }
      for (const [from, to] of plan.filePathRemaps) {
        remapFileNonViewportUiState(from, to);
        assertCurrentProjectIdentity(prepared);
        if (!plan.index.charts.some((chart) => chart.chartPath === to)) continue;
        invalidateChartPreviewCacheForMove(plan.projectInstanceId, from, to);
      }
      const focus = useEditorStore.getState().detailFocus;
      const focusedResource = detailResource(focus, plan.storeState.resources);
      if (
        detailResourceRef(focus) &&
        (!focusedResource || !shouldRetainResourceEditor(focusedResource))
      ) {
        useEditorStore.getState().clearDetailFocus();
        assertCurrentProjectIdentity(prepared);
      }
    },
    () => isCurrentProjectIdentity(prepared),
  );
}

/** Only explicit delete receipts authorize discarding dirty resources absent from the index. */
export function collectDeletedResourceKeys(
  index: ProjectIndexRow,
  receipts: readonly ResourceMutationResultDto[],
): ReadonlySet<ResourceKey> {
  const present = new Set([
    ...nodeFileEntries(index).map((file) => resourceKey({ id: file.path, kind: file.type })),
    ...index.charts.map((file) => resourceKey({ id: file.chartPath, kind: "chart" })),
    ...index.minds.map((file) => resourceKey({ id: file.path, kind: "mind" })),
    ...index.docs.map((file) => resourceKey({ id: file.path, kind: "doc" })),
  ]);
  const deleted = new Set<ResourceKey>();
  for (const receipt of receipts)
    for (const delta of receipt.deltas) {
      if (delta.payload.kind !== "resource_lifecycle") continue;
      const { before, after } = delta.payload.patch;
      if (!before || after) continue;
      const key = resourceKey({ id: before.path, kind: before.kind });
      if (!present.has(key)) deleted.add(key);
    }
  return deleted;
}
