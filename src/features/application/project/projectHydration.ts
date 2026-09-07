import { useProjectIOStore, resetGraphLoadOwnership } from "./projectIOStore";
import { LoadStatus } from "@/shared/types/ui/common";
import { ProjectService, type ProjectActivationResult } from "@/services/project/projectService";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { logger } from "@/features/application/observability/appLogger";

import type { DatabaseRecord } from "@/shared/types/domain/database";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";

import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import {
  buildGraphResourceMeta,
  useResourceStore,
  type ProjectResourceMeta,
} from "@/features/core/resource";
import {
  applySnapshotDocumentPatches,
  prepareResourceProjectionSnapshot,
} from "@/features/core/resource/resourceSnapshotProjection";
import { resetClientProjectState } from "@/features/application/project/projectReset";
import { synchronizeProjectPresentation } from "@/features/application/project/projectPresentationSync";
import { removeProjectScopedWorkbenchPanels } from "@/features/application/project/projectWorkbenchLifecycle";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { resetFunctionSignatureCoordinator } from "@/features/application/editorMutation/functionSignatureCoordinator";
import { resetHistoryCoordinator } from "@/features/application/graphDraft/historyCoordinator";
import { resetGraphDraftCoordinator } from "@/features/application/graphDraft/graphDraftCoordinator";
import { resetGraphProjectionLifecycle } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { hydrateFunctionSignaturesFromProjectIndex } from "@/features/application/graphDocument/functionSignatureSync";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { useDocumentStateStore } from "@/features/core/resource/documentStateStore";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { useViewportStore } from "@/features/core/viewport";
import { useGraphInteractionStore } from "@/features/core/graphInteraction";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { useColumnStatsStore } from "@/features/core/dataStore/columnStatsStore";
import { useColumnDistributionStore } from "@/features/core/dataStore/columnDistributionStore";
import { useDatasetOverviewStore } from "@/features/core/dataStore/datasetOverviewStore";
import {
  buildAuthoritativeProjectLoadPlan,
  defaultAuthoritativeProjectLoadPlanDependencies,
  type AuthoritativeProjectLoadPlanDependencies,
  type PreparedAuthoritativeProjectLoad as BasePreparedAuthoritativeProjectLoad,
} from "@/features/application/project/authoritativeProjectLoadPlan";
import {
  ProjectLifecycleError,
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
  isProjectLifecycleStateCurrent,
  type ProjectIdentitySnapshot,
  type ProjectLifecycleStateSnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

export interface ProjectLoadReceipt {
  readonly projectInstanceId: string;
  readonly publicationRevision: number;
}

export type { AuthoritativeProjectLoadPlanDependencies } from "@/features/application/project/authoritativeProjectLoadPlan";
export type PreparedAuthoritativeProjectLoad = BasePreparedAuthoritativeProjectLoad & {
  readonly identity: ProjectIdentitySnapshot;
};

export const PROJECT_LOAD_CONTRACT_ERROR_CODE = "project_load_contract_error";
export const PROJECT_RESOURCE_INDEX_CONTRACT_ERROR_CODE = "project_resource_index_contract_error";
const PROJECT_LOAD_COMMIT_ERROR_CODE = "project_load_commit_error";

function errorReferenceForLog(reference: ErrorReference): string {
  return reference.incidentId
    ? `[${reference.code}] incidentId=${reference.incidentId}`
    : `[${reference.code}]`;
}

function logProjectIOError(context: string, reference: ErrorReference): void {
  try {
    logger.sys.error(`${context} ${errorReferenceForLog(reference)}`, "ProjectIOStore");
  } catch {
    // Diagnostics must not control project state transitions.
  }
}

function buildResourceIndex(params: {
  graphs: Array<{ path: string; name: string; type: "event" | "function"; revision?: number }>;
  charts: Array<{
    chartPath: string;
    name: string;
    databaseId: string;
    chartType: import("@/shared/types/domain/chart").ChartType;
    revision: number;
  }>;
  databases: Record<string, DatabaseRecord>;
}): ProjectResourceMeta[] {
  const resources: ProjectResourceMeta[] = [];
  for (const graph of params.graphs) {
    resources.push(
      buildGraphResourceMeta(graph.type, graph.path, graph.name, {
        revision: graph.revision,
      }),
    );
  }
  for (const chart of params.charts) {
    resources.push({
      id: chart.chartPath,
      kind: "chart",
      name: chart.name,
      uri: `yssbi://chart/${chart.chartPath}`,
      revision: chart.revision,
      exists: true,
      loaded: Boolean(useChartDocumentStore.getState().documents[chart.chartPath]),
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  for (const [id, database] of Object.entries(params.databases)) {
    const name = typeof database.name === "string" ? database.name : id;
    resources.push({
      id,
      kind: "database",
      name,
      uri: `yssbi://database/${id}`,
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  return resources;
}

let refreshResourceIndexInFlight: Promise<boolean> | null = null;
let refreshResourceIndexPending = false;

export async function refreshProjectResourceIndex(): Promise<boolean> {
  if (refreshResourceIndexInFlight) {
    refreshResourceIndexPending = true;
    return refreshResourceIndexInFlight;
  }

  refreshResourceIndexInFlight = (async () => {
    let lastResult = false;
    try {
      do {
        refreshResourceIndexPending = false;
        lastResult = await refreshProjectResourceIndexOnce();
      } while (refreshResourceIndexPending);
      return lastResult;
    } finally {
      refreshResourceIndexInFlight = null;
    }
  })();

  return refreshResourceIndexInFlight;
}

async function refreshProjectResourceIndexOnce(): Promise<boolean> {
  const identity = captureProjectIdentity();
  try {
    const index = await ProjectService.getProjectIndex(identity.projectInstanceId);
    if (!isCurrentProjectIdentity(identity)) return false;
    if (index.projectInstanceId !== identity.projectInstanceId) return false;

    const databaseRows = index.databases;
    const databasePaths = Object.fromEntries(databaseRows.map((row) => [row.id, row.resourcePath]));
    useDatabaseStore.setState((state) => ({
      databases: Object.fromEntries(
        Object.entries(state.databases).map(([id, database]) => [
          id,
          { ...database, resourcePath: databasePaths[id] },
        ]),
      ),
      revisions: Object.fromEntries(databaseRows.map((row) => [row.id, row.revision])),
    }));

    const graphOrder = index.graphs.map((graph) => graph.path);

    const chartIndex = index.charts.map((chart) => ({
      chartPath: chart.chartPath,
      name: chart.name,
      databaseId: chart.databaseId,
      chartType: chart.chartType as import("@/shared/types/domain/chart").ChartType,
      revision: chart.revision,
    }));
    useChartDocumentStore.getState().setIndex(chartIndex);

    const incoming = buildResourceIndex({
      graphs: index.graphs,
      charts: chartIndex,
      databases: useDatabaseStore.getState().databases,
    });

    const previousByKey = useResourceStore.getState().resources;
    const { resources, documentPatches } = prepareResourceProjectionSnapshot(
      incoming,
      previousByKey,
    );
    applySnapshotDocumentPatches(documentPatches);

    useResourceStore.getState().setSnapshot({
      resources,
      graphOrder,
    });
    hydrateFunctionSignaturesFromProjectIndex(index.graphs);
    synchronizeProjectPresentation();
    return true;
  } catch (err) {
    if (!isCurrentProjectIdentity(identity)) return false;
    const error = toErrorReference(err, PROJECT_RESOURCE_INDEX_CONTRACT_ERROR_CODE);
    useProjectIOStore.setState({ error });
    logProjectIOError("Failed to refresh resource index", error);
    return false;
  }
}

export async function prepareAuthoritativeProjectLoad(
  identity: ProjectIdentitySnapshot,
  dependencyOverrides: Partial<AuthoritativeProjectLoadPlanDependencies> = {},
): Promise<PreparedAuthoritativeProjectLoad> {
  const path = await ProjectService.getProjectPath(identity.projectInstanceId);
  assertCurrentProjectIdentity(identity);
  const { databases } = await ProjectService.getDatabases(identity.projectInstanceId);
  assertCurrentProjectIdentity(identity);
  const index = await ProjectService.getProjectIndex(identity.projectInstanceId);
  assertCurrentProjectIdentity(identity);
  if (index.projectInstanceId !== identity.projectInstanceId) {
    throw new Error("Project index identity does not match the requested project");
  }
  const prepared = buildAuthoritativeProjectLoadPlan(
    { path, databases, index },
    {
      databases: useDatabaseStore.getState().databases,
      detailFocus: useEditorStore.getState().detailFocus,
    },
    {
      ...defaultAuthoritativeProjectLoadPlanDependencies,
      validateCoordinatorStart: (projectInstanceId, publicationRevision) => {
        projectPublicationCoordinator.validateProjectStart(projectInstanceId, publicationRevision);
      },
      ...dependencyOverrides,
    },
  );
  return { ...prepared, identity };
}

function commitProjectLoadStep(label: string, assignment: () => void): void {
  try {
    assignment();
  } catch (error) {
    logProjectIOError(
      `Project load commit listener failed at '${label}'`,
      toErrorReference(error, PROJECT_LOAD_COMMIT_ERROR_CODE),
    );
  }
}

export async function commitPreparedAuthoritativeProjectLoad(
  prepared: PreparedAuthoritativeProjectLoad,
): Promise<ProjectLoadReceipt> {
  assertCurrentProjectIdentity(prepared.identity);
  const previousProjectInstanceId = useProjectIOStore.getState().projectInstanceId;
  const nextProjectInstanceId = prepared.index.projectInstanceId;
  const isProjectReplacement =
    previousProjectInstanceId !== null && previousProjectInstanceId !== nextProjectInstanceId;
  if (isProjectReplacement) {
    await removeProjectScopedWorkbenchPanels(previousProjectInstanceId, prepared.identity);
  }
  assertCurrentProjectIdentity(prepared.identity);
  if (useProjectIOStore.getState().projectInstanceId !== previousProjectInstanceId) {
    throw new ProjectLifecycleError();
  }

  projectPublicationCoordinator.startProject(
    nextProjectInstanceId,
    prepared.index.publicationRevision,
  );
  commitProjectLoadStep("graph projection lifecycle", resetGraphProjectionLifecycle);
  commitProjectLoadStep("graph draft coordinator", resetGraphDraftCoordinator);
  resetGraphLoadOwnership();
  commitProjectLoadStep("graph load status", () =>
    useProjectIOStore.setState({
      graphLoadStatus: {},
    }),
  );
  commitProjectLoadStep("function signature coordinator", resetFunctionSignatureCoordinator);
  commitProjectLoadStep("history coordinator", resetHistoryCoordinator);

  commitProjectLoadStep("detail focus", () =>
    useEditorStore.setState({
      detailFocus: isProjectReplacement ? null : prepared.storeState.detailFocus,
    }),
  );
  commitProjectLoadStep("viewport", () => useViewportStore.setState({ viewports: {} }));
  commitProjectLoadStep("graph interaction", () =>
    useGraphInteractionStore.setState({
      positionOverrides: {},
    }),
  );
  commitProjectLoadStep("column stats", () =>
    useColumnStatsStore.setState({ statsByDatabase: {} }),
  );
  commitProjectLoadStep("column distribution", () =>
    useColumnDistributionStore.setState({
      distByDatabase: {},
    }),
  );
  commitProjectLoadStep("dataset overview", () =>
    useDatasetOverviewStore.setState({
      overviewByDatabase: {},
    }),
  );
  commitProjectLoadStep("database", () =>
    useDatabaseStore.setState({
      databases: prepared.storeState.databases,
      revisions: prepared.storeState.databaseRevisions,
    }),
  );
  commitProjectLoadStep("chart", () =>
    useChartDocumentStore.setState({
      index: prepared.storeState.chartIndex,
      documents: {},
    }),
  );
  commitProjectLoadStep("documents", () => useDocumentStateStore.setState({ documents: {} }));
  commitProjectLoadStep("resources", () =>
    useResourceStore.setState({
      resources: prepared.storeState.resources,
      graphOrder: prepared.storeState.graphOrder,
    }),
  );
  commitProjectLoadStep("function metadata", () =>
    useGraphMetaStore.setState({
      graphs: prepared.storeState.graphMeta,
    }),
  );
  commitProjectLoadStep("graph session", () =>
    useGraphSessionStore.setState({ focusedSession: null }),
  );
  commitProjectLoadStep("graph data", () =>
    useGraphProjectionStore.setState({ graphEntities: {} }),
  );
  commitProjectLoadStep("project IO", () =>
    useProjectIOStore.setState(prepared.storeState.projectIO),
  );
  commitProjectLoadStep("open panel synchronization", synchronizeProjectPresentation);
  commitProjectLoadStep("completion log", () => {
    logger.sys.info("Project loaded (index from Rust)", "ProjectIOStore");
  });
  return {
    projectInstanceId: prepared.index.projectInstanceId,
    publicationRevision: prepared.index.publicationRevision,
  };
}

/** Identity-keyed hydration prevents an old in-flight load from absorbing a replacement project. */
let loadProjectInFlight: {
  key: string;
  promise: Promise<ProjectLoadReceipt | null>;
} | null = null;

function identityKey(identity: ProjectIdentitySnapshot): string {
  return `${identity.projectInstanceId}:${identity.epoch}`;
}

async function loadProjectForIdentity(
  identity: ProjectIdentitySnapshot,
): Promise<ProjectLoadReceipt | null> {
  const key = identityKey(identity);
  if (loadProjectInFlight?.key === key) return loadProjectInFlight.promise;

  const entry = {
    key,
    promise: Promise.resolve<ProjectLoadReceipt | null>(null),
  };
  entry.promise = (async () => {
    useProjectIOStore.setState({ status: LoadStatus.Loading, error: null });
    try {
      const prepared = await prepareAuthoritativeProjectLoad(identity);
      assertCurrentProjectIdentity(identity);
      return await commitPreparedAuthoritativeProjectLoad(prepared);
    } catch (err) {
      if (!isCurrentProjectIdentity(identity)) return null;
      const error = toErrorReference(err, PROJECT_LOAD_CONTRACT_ERROR_CODE);
      useProjectIOStore.setState({ status: LoadStatus.Error, error });
      logProjectIOError("Failed to load project", error);
      return null;
    } finally {
      if (loadProjectInFlight === entry) loadProjectInFlight = null;
    }
  })();
  loadProjectInFlight = entry;
  return entry.promise;
}

export function loadActivatedProject(
  activation: ProjectActivationResult,
): Promise<ProjectLoadReceipt | null> {
  if (
    !projectPublicationCoordinator.acceptProjectActivation(
      activation.projectInstanceId,
      activation.activationRevision,
    )
  ) {
    return Promise.resolve(null);
  }
  return loadProjectForIdentity(captureProjectIdentity());
}

export function loadCurrentProject(): Promise<ProjectLoadReceipt | null> {
  return loadProjectForIdentity(captureProjectIdentity());
}

export async function clearProjectProjection(owner: ProjectLifecycleStateSnapshot): Promise<void> {
  if (!isProjectLifecycleStateCurrent(owner)) return;
  const previousProjectInstanceId = useProjectIOStore.getState().projectInstanceId;
  await resetClientProjectState(previousProjectInstanceId, owner, {
    removeProjectScopedWorkbenchPanels,
  });

  let expectedProjectInstanceId = previousProjectInstanceId;
  const commitOwnedClear = (assignment: () => void): boolean => {
    if (!isProjectLifecycleStateCurrent(owner)) return false;
    if (useProjectIOStore.getState().projectInstanceId !== expectedProjectInstanceId) return false;
    assignment();
    return true;
  };

  if (!commitOwnedClear(resetGraphProjectionLifecycle)) return;
  if (!commitOwnedClear(resetGraphDraftCoordinator)) return;
  if (!commitOwnedClear(() => resetGraphLoadOwnership())) return;
  if (!commitOwnedClear(() => useProjectIOStore.setState({ graphLoadStatus: {} }))) return;
  if (!commitOwnedClear(() => useProjectIOStore.setState({ projectInstanceId: null }))) return;
  expectedProjectInstanceId = null;
  if (
    !commitOwnedClear(() => {
      useGraphProjectionStore.setState({ graphEntities: {} });
    })
  )
    return;

  if (!commitOwnedClear(() => useDatabaseStore.getState().setDatabaseSnapshot({}, {}))) return;
  if (
    !commitOwnedClear(() =>
      useResourceStore.getState().setSnapshot({ resources: [], graphOrder: [] }),
    )
  )
    return;
  if (!commitOwnedClear(synchronizeProjectPresentation)) return;
  commitOwnedClear(() => {
    useProjectIOStore.setState({ status: LoadStatus.Ready, currentPath: null });
  });
}
