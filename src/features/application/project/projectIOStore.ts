import { createBoundApplicationStore } from "@/features/core/state/applicationStore";
import { LoadStatus } from "@/shared/types/ui/common";
import type { ProjectData, Variable } from "@/shared/types";
import { ProjectService, type ProjectActivationResult } from "@/services/project/projectService";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { normalizeVariableFromBackend } from "@/shared/types/domain/variable";
import { logger } from "@/features/application/observability/appLogger";

import { normalizeDatabases } from "@/features/application/dataManagement/databaseRecords";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { useVariableStore } from "@/features/core/dataStore/variableStore";
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
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableRevisionsFromIndex,
} from "@/features/core/variable/variableCatalog";
import { resetClientProjectState } from "@/features/application/project/projectReset";
import { synchronizeProjectPresentation } from "@/features/application/project/projectPresentationSync";
import { removeProjectScopedWorkbenchPanels } from "@/features/application/project/projectWorkbenchLifecycle";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { resetFunctionSignatureCoordinator } from "@/features/application/editorMutation/functionSignatureCoordinator";
import { resetHistoryCoordinator } from "@/features/application/graphDraft/historyCoordinator";
import { resetGraphDraftCoordinator } from "@/features/application/graphDraft/graphDraftCoordinator";
import { resetGraphProjectionCoordinator } from "@/features/application/graphProjection/graphProjectionCoordinator";
import {
  beginGraphLoadLifecycle,
  loadGraphProjection,
  resetGraphProjectionLifecycle,
} from "@/features/application/graphProjection/graphProjectionLifecycle";
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
import { useHistoryStore } from "@/features/core/history";
import { isGraphCachedInMemory } from "@/features/core/dataStore/graphDocumentLoadPolicy";
import { setProjectPathForViewport } from "@/features/core/viewport/projectPath";

export type { AuthoritativeProjectLoadPlanDependencies } from "@/features/application/project/authoritativeProjectLoadPlan";
export type PreparedAuthoritativeProjectLoad = BasePreparedAuthoritativeProjectLoad & {
  readonly identity: ProjectIdentitySnapshot;
};

export type GraphLoadStatus = "loading" | "ready" | "error";

export const PROJECT_LOAD_CONTRACT_ERROR_CODE = "project_load_contract_error";
export const PROJECT_RESOURCE_INDEX_CONTRACT_ERROR_CODE = "project_resource_index_contract_error";
export const GRAPH_PROJECTION_CONTRACT_ERROR_CODE = "graph_projection_contract_error";
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

interface ProjectIOStore {
  status: LoadStatus;
  error: ErrorReference | null;
  graphLoadStatus: Record<string, GraphLoadStatus>;

  currentPath: string | null;
  projectInstanceId: string | null;

  setCurrentPath(path: string | null): void;
  /** 从 Rust 当前项目状态拉取并灌入前端 store（路径、变量、库、图索引）；合并并发调用 */
  loadProject(): Promise<ProjectData | null>;
  /** 只刷新资源索引，不重置 tab、viewport、history 或已加载 graph 正文。 */
  refreshResourceIndex(): Promise<boolean>;
  loadProjectFromData(
    project: ProjectData,
    path: string | null,
    owner: ProjectLifecycleStateSnapshot,
  ): Promise<void>;
  loadGraph(graphPath: string): Promise<boolean>;
}

/** 将后端变量 DTO 规范化为前端 Variable */
function normalizeVariables(
  vars: Record<string, Variable | Record<string, unknown>>,
): Record<string, Variable> {
  const result: Record<string, Variable> = {};
  for (const [id, v] of Object.entries(vars)) {
    const raw = typeof v === "object" && v !== null ? { ...v, id } : { id };
    result[id] = normalizeVariableFromBackend(
      raw as Parameters<typeof normalizeVariableFromBackend>[0],
    );
  }
  return result;
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
  variables: Record<string, Variable>;
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
  resources.push(...variableCatalogToResourceMetas(params.variables));
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

async function refreshProjectResourceIndex(): Promise<boolean> {
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
    const variableCatalog = applyVariableCatalogFromIndex(index.variables);
    useVariableStore
      .getState()
      .setVariableSnapshot(variableCatalog, variableRevisionsFromIndex(index.variables));

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
      variables: variableCatalog,
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

/** 将后端/快照 databases 规范化并写入 store（合并已有富元数据） */
function applyDatabasesFromRaw(raw: Record<string, unknown>): Record<string, DatabaseRecord> {
  const normalized = normalizeDatabases(raw, useDatabaseStore.getState().databases);
  useDatabaseStore.getState().setDatabaseSnapshot(normalized, {});
  return normalized;
}

export async function prepareAuthoritativeProjectLoad(
  identity: ProjectIdentitySnapshot,
  dependencyOverrides: Partial<AuthoritativeProjectLoadPlanDependencies> = {},
): Promise<PreparedAuthoritativeProjectLoad> {
  const path = await ProjectService.getProjectPath(identity.projectInstanceId);
  assertCurrentProjectIdentity(identity);
  const { databases } = await ProjectService.getDatabasesVariables(identity.projectInstanceId);
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
): Promise<ProjectData> {
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
  commitProjectLoadStep("graph projection channel coordinator", resetGraphProjectionCoordinator);
  commitProjectLoadStep("graph draft coordinator", resetGraphDraftCoordinator);
  loadGraphInFlight.clear();
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
  commitProjectLoadStep("variable", () =>
    useVariableStore.setState({
      variables: prepared.storeState.variables,
      revisions: prepared.storeState.variableRevisions,
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
  commitProjectLoadStep("history", () => useHistoryStore.setState(prepared.storeState.history));
  commitProjectLoadStep("project IO", () =>
    useProjectIOStore.setState(prepared.storeState.projectIO),
  );
  commitProjectLoadStep("open panel synchronization", synchronizeProjectPresentation);
  commitProjectLoadStep("completion log", () => {
    logger.sys.info("Project loaded (index from Rust)", "ProjectIOStore");
  });
  return prepared.projectData;
}

/** Identity-keyed hydration prevents an old in-flight load from absorbing a replacement project. */
let loadProjectInFlight: {
  key: string;
  promise: Promise<ProjectData | null>;
} | null = null;

function identityKey(identity: ProjectIdentitySnapshot): string {
  return `${identity.projectInstanceId}:${identity.epoch}`;
}

async function loadProjectForIdentity(
  identity: ProjectIdentitySnapshot,
): Promise<ProjectData | null> {
  const key = identityKey(identity);
  if (loadProjectInFlight?.key === key) return loadProjectInFlight.promise;

  const entry = {
    key,
    promise: Promise.resolve<ProjectData | null>(null),
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
): Promise<ProjectData | null> {
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

interface GraphLoadInFlight {
  lifecycleToken: number;
  promise: Promise<boolean>;
}

const loadGraphInFlight = new Map<string, GraphLoadInFlight>();

export function invalidateGraphLoadOwnership(graphPath: string): void {
  loadGraphInFlight.delete(graphPath);
}

export const useProjectIOStore = createBoundApplicationStore<ProjectIOStore>((set, get) => ({
  status: LoadStatus.Idle,
  error: null,
  graphLoadStatus: {},

  currentPath: null,
  projectInstanceId: null,

  setCurrentPath: (path) => set({ currentPath: path ? formatDisplayPath(path) : null }),

  loadProject: async () => loadProjectForIdentity(captureProjectIdentity()),

  refreshResourceIndex: refreshProjectResourceIndex,

  loadProjectFromData: async (project, path, owner) => {
    if (!isProjectLifecycleStateCurrent(owner)) return;
    const previousProjectInstanceId = get().projectInstanceId;
    await resetClientProjectState(previousProjectInstanceId, owner, {
      removeProjectScopedWorkbenchPanels,
    });

    let expectedProjectInstanceId = previousProjectInstanceId;
    const commitOwnedClear = (assignment: () => void): boolean => {
      if (!isProjectLifecycleStateCurrent(owner)) return false;
      if (get().projectInstanceId !== expectedProjectInstanceId) return false;
      assignment();
      return true;
    };

    if (!commitOwnedClear(resetGraphProjectionLifecycle)) return;
    if (!commitOwnedClear(resetGraphProjectionCoordinator)) return;
    if (!commitOwnedClear(resetGraphDraftCoordinator)) return;
    if (!commitOwnedClear(() => loadGraphInFlight.clear())) return;
    if (!commitOwnedClear(() => set({ graphLoadStatus: {} }))) return;
    if (!commitOwnedClear(() => set({ projectInstanceId: null }))) return;
    expectedProjectInstanceId = null;
    if (
      !commitOwnedClear(() => {
        useGraphProjectionStore.setState({ graphEntities: {} });
      })
    )
      return;

    const normalizedVariables = normalizeVariables(project.variables);
    let normalizedDatabases: Record<string, DatabaseRecord> = {};
    if (
      !commitOwnedClear(() => {
        normalizedDatabases = applyDatabasesFromRaw(project.databases as Record<string, unknown>);
      })
    )
      return;
    if (
      !commitOwnedClear(() => {
        useVariableStore.getState().setVariables(normalizedVariables);
      })
    )
      return;
    if (
      !commitOwnedClear(() => {
        useResourceStore.getState().setSnapshot({
          resources: buildResourceIndex({
            graphs: Object.values(project.graphs).map((graph) => ({
              path: graph.path,
              name: graph.name,
              type: graph.type,
            })),
            charts: [],
            variables: normalizedVariables,
            databases: normalizedDatabases,
          }),
          graphOrder: Object.values(project.graphs).map((graph) => graph.path),
        });
      })
    )
      return;
    if (!commitOwnedClear(synchronizeProjectPresentation)) return;
    commitOwnedClear(() => {
      set({ status: LoadStatus.Ready, currentPath: path ? formatDisplayPath(path) : null });
    });
  },

  loadGraph: async (graphPath) => {
    if (isGraphCachedInMemory(graphPath)) {
      set((state) => ({
        graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: "ready" },
      }));
      return true;
    }

    const existing = loadGraphInFlight.get(graphPath);
    if (existing) return existing.promise;

    set((state) => ({
      graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: "loading" },
    }));
    const lifecycleToken = beginGraphLoadLifecycle(graphPath);
    const pending = loadGraphProjection(graphPath, lifecycleToken)
      .catch((err) => {
        const error = toErrorReference(err, GRAPH_PROJECTION_CONTRACT_ERROR_CODE);
        set({ error });
        logProjectIOError("Failed to load graph projection", error);
        return false;
      })
      .then((loaded) => {
        const current = loadGraphInFlight.get(graphPath);
        if (current?.lifecycleToken === lifecycleToken && current.promise === pending) {
          set((state) => ({
            graphLoadStatus: {
              ...state.graphLoadStatus,
              [graphPath]: loaded ? "ready" : "error",
            },
          }));
        }
        return loaded;
      })
      .finally(() => {
        const current = loadGraphInFlight.get(graphPath);
        if (current?.lifecycleToken === lifecycleToken && current.promise === pending) {
          loadGraphInFlight.delete(graphPath);
        }
      });

    loadGraphInFlight.set(graphPath, { lifecycleToken, promise: pending });
    return pending;
  },
}));

setProjectPathForViewport(useProjectIOStore.getState().currentPath);
useProjectIOStore.subscribe((state) => setProjectPathForViewport(state.currentPath));
