import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import type { ProjectData, Variable } from '@/shared/types';
import {
  ProjectService,
  type ProjectActivationResult,
} from '@/services/project/projectService';
import { toErrorReference, type ErrorReference } from '@/services/ipc';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { logger } from '@/utils/appLogger';

import type { DatabaseRecord } from '@/shared/types/dto/database';
import { normalizeDatabases } from '@/shared/types/dto/database';
import { useVariableStore } from './variableStore';
import { useDatabaseStore } from './databaseStore';
import { useGraphDataStore } from './graphDataStore';

import { projectIOApplicationPort } from './projectIOApplicationPort';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { buildGraphResourceMeta, useResourceStore, type ProjectResourceMeta } from '@/features/core/resource';
import {
  applySnapshotDocumentPatches,
  reconcileResourceSnapshot,
} from '@/features/core/resource/resourceSnapshotReconcile';
import { formatDisplayPath } from '@/shared/utils/formatDisplayPath';
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableRevisionsFromIndex,
} from '@/features/core/variable/variableCatalog';
import {
  resetClientProjectState,
  resetProjectScopedRightSidebarState,
} from './projectClientReset';
import { useGraphMetaStore } from './graphMetaStore';
import { useDocumentStateStore } from '@/features/core/resource/documentStateStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { editorDockviewPort, useEditorPaneStateStore } from '@/features/core/dockview';
import { useViewportStore } from '@/features/core/viewport';
import { useGraphInteractionStore } from '@/features/core/graphInteraction';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useEditStateStore } from './editStateStore';
import { useColumnStatsStore } from './columnStatsStore';
import { useColumnDistributionStore } from './columnDistributionStore';
import { useDatasetOverviewStore } from './datasetOverviewStore';
import {
  buildAuthoritativeProjectLoadPlan,
  defaultAuthoritativeProjectLoadPlanDependencies,
  type AuthoritativeProjectLoadPlanDependencies,
  type PreparedAuthoritativeProjectLoad,
} from './authoritativeProjectLoadPlan';
export type {
  AuthoritativeProjectLoadPlanDependencies,
  PreparedAuthoritativeProjectLoad,
} from './authoritativeProjectLoadPlan';

import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { useHistoryStore } from '@/features/core/history';
import { isGraphCachedInMemory } from './graphDocumentLoadPolicy';


export type GraphLoadStatus = 'loading' | 'ready' | 'error';

export const PROJECT_LOAD_CONTRACT_ERROR_CODE = 'project_load_contract_error';
export const PROJECT_RESOURCE_INDEX_CONTRACT_ERROR_CODE = 'project_resource_index_contract_error';
export const GRAPH_PROJECTION_CONTRACT_ERROR_CODE = 'graph_projection_contract_error';
const PROJECT_LOAD_COMMIT_ERROR_CODE = 'project_load_commit_error';

function errorReferenceForLog(reference: ErrorReference): string {
  return reference.incidentId
    ? `[${reference.code}] incidentId=${reference.incidentId}`
    : `[${reference.code}]`;
}

function logProjectIOError(context: string, reference: ErrorReference): void {
  try {
    logger.sys.error(`${context} ${errorReferenceForLog(reference)}`, 'ProjectIOStore');
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
  loadProjectFromData(project: ProjectData, path: string | null): void;
  loadGraph(graphPath: string): Promise<boolean>;
}

/** 将后端变量 DTO 规范化为前端 Variable */
function normalizeVariables(
  vars: Record<string, Variable | Record<string, unknown>>
): Record<string, Variable> {
  const result: Record<string, Variable> = {};
  for (const [id, v] of Object.entries(vars)) {
    const raw = typeof v === 'object' && v !== null ? { ...v, id } : { id };
    result[id] = normalizeVariableFromBackend(raw as Parameters<typeof normalizeVariableFromBackend>[0]);
  }
  return result;
}

function buildResourceIndex(params: {
  graphs: Array<{ path: string; name: string; type: 'event' | 'function'; revision?: number }>;
  worksheets: Array<{
    worksheetPath: string;
    name: string;
    databaseId: string;
    chartType: import('@/shared/types/domain/worksheet').WorksheetChartType;
    revision: number;
  }>;
  variables: Record<string, Variable>;
  databases: Record<string, DatabaseRecord>;
}): ProjectResourceMeta[] {
  const resources: ProjectResourceMeta[] = [];
  for (const graph of params.graphs) {
    resources.push(buildGraphResourceMeta(graph.type, graph.path, graph.name, {
      revision: graph.revision,
    }));
  }
  for (const worksheet of params.worksheets) {
    resources.push({
      id: worksheet.worksheetPath,
      kind: 'worksheet',
      name: worksheet.name,
      uri: `yssbi://worksheet/${worksheet.worksheetPath}`,
      revision: worksheet.revision,
      exists: true,
      loaded: Boolean(useWorksheetStore.getState().documents[worksheet.worksheetPath]),
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  resources.push(...variableCatalogToResourceMetas(params.variables));
  for (const [id, database] of Object.entries(params.databases)) {
    const name = typeof database.name === 'string' ? database.name : id;
    resources.push({
      id,
      kind: 'database',
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
    useVariableStore.getState().setVariableSnapshot(
      variableCatalog,
      variableRevisionsFromIndex(index.variables),
    );

    const databaseRows = index.databases;
    const databasePaths = Object.fromEntries(
      databaseRows.map((row) => [row.id, row.resourcePath]),
    );
    useDatabaseStore.setState((state) => ({
      databases: Object.fromEntries(Object.entries(state.databases).map(([id, database]) => [
        id,
        { ...database, resourcePath: databasePaths[id] },
      ])),
      revisions: Object.fromEntries(databaseRows.map((row) => [row.id, row.revision])),
    }));

    const graphOrder = index.graphs.map((graph) => graph.path);

    const worksheetIndex = index.worksheets.map((worksheet) => ({
      worksheetPath: worksheet.worksheetPath,
      name: worksheet.name,
      databaseId: worksheet.databaseId,
      chartType: worksheet.chartType as import('@/shared/types/domain/worksheet').WorksheetChartType,
      revision: worksheet.revision,
    }));
    useWorksheetStore.getState().setIndex(worksheetIndex);

    const incoming = buildResourceIndex({
      graphs: index.graphs,
      worksheets: worksheetIndex,
      variables: variableCatalog,
      databases: useDatabaseStore.getState().databases,
    });

    const previousByKey = useResourceStore.getState().resources;
    const { resources, documentPatches } = reconcileResourceSnapshot(incoming, previousByKey);
    applySnapshotDocumentPatches(documentPatches);

    useResourceStore.getState().setSnapshot({
      resources,
      graphOrder,
    });
    projectIOApplicationPort().hydrateFunctionSignatures(index.graphs);
    projectIOApplicationPort().reconcileOpenTabs();
    return true;
  } catch (err) {
    if (!isCurrentProjectIdentity(identity)) return false;
    const error = toErrorReference(err, PROJECT_RESOURCE_INDEX_CONTRACT_ERROR_CODE);
    useProjectIOStore.setState({ error });
    logProjectIOError('Failed to refresh resource index', error);
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
    throw new Error('Project index identity does not match the requested project');
  }
  return buildAuthoritativeProjectLoadPlan(
    { path, databases, index },
    {
      databases: useDatabaseStore.getState().databases,
      detailFocus: useEditorStore.getState().detailFocus,
    },
    {
      ...defaultAuthoritativeProjectLoadPlanDependencies,
      validateCoordinatorStart: (projectInstanceId, publicationRevision) => {
        projectIOApplicationPort().validatePublicationStart(projectInstanceId, publicationRevision);
      },
      ...dependencyOverrides,
    },
  );
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

export function commitPreparedAuthoritativeProjectLoad(
  prepared: PreparedAuthoritativeProjectLoad,
): ProjectData {
  projectIOApplicationPort().startPublication(
    prepared.index.projectInstanceId,
    prepared.index.publicationRevision,
  );
  commitProjectLoadStep('graph projection coordinator', () => projectIOApplicationPort().resetGraphProjection());
  loadGraphInFlight.clear();
  commitProjectLoadStep('graph load status', () => useProjectIOStore.setState({
    graphLoadStatus: {},
  }));
  commitProjectLoadStep('function signature coordinator', () => projectIOApplicationPort().resetFunctionSignatures());
  commitProjectLoadStep('history coordinator', () => projectIOApplicationPort().resetHistory());

  commitProjectLoadStep('editor dock', () => { void editorDockviewPort.reset(); });
  commitProjectLoadStep('editor pane state', () => useEditorPaneStateStore.getState().reset());
  commitProjectLoadStep('project-scoped right sidebar', resetProjectScopedRightSidebarState);
  commitProjectLoadStep('detail focus', () => useEditorStore.setState({
    detailFocus: prepared.storeState.detailFocus,
  }));
  commitProjectLoadStep('viewport', () => useViewportStore.setState({ viewports: {} }));
  commitProjectLoadStep('graph interaction', () => useGraphInteractionStore.setState({
    positionOverrides: {},
  }));
  commitProjectLoadStep('edit state', () => useEditStateStore.setState({ editStateByDatabase: {} }));
  commitProjectLoadStep('column stats', () => useColumnStatsStore.setState({ statsByDatabase: {} }));
  commitProjectLoadStep('column distribution', () => useColumnDistributionStore.setState({
    distByDatabase: {},
  }));
  commitProjectLoadStep('dataset overview', () => useDatasetOverviewStore.setState({
    overviewByDatabase: {},
  }));
  commitProjectLoadStep('database', () => useDatabaseStore.setState({
    databases: prepared.storeState.databases,
    revisions: prepared.storeState.databaseRevisions,
  }));
  commitProjectLoadStep('variable', () => useVariableStore.setState({
    variables: prepared.storeState.variables,
    revisions: prepared.storeState.variableRevisions,
  }));
  commitProjectLoadStep('worksheet', () => useWorksheetStore.setState({
    index: prepared.storeState.worksheetIndex,
    documents: {},
  }));
  commitProjectLoadStep('documents', () => useDocumentStateStore.setState({ documents: {} }));
  commitProjectLoadStep('resources', () => useResourceStore.setState({
    resources: prepared.storeState.resources,
    graphOrder: prepared.storeState.graphOrder,
  }));
  commitProjectLoadStep('function metadata', () => useGraphMetaStore.setState({
    graphs: prepared.storeState.graphMeta,
  }));
  commitProjectLoadStep('graph session', () => useGraphSessionStore.setState({ focusedSession: null }));
  commitProjectLoadStep('graph data', () => useGraphDataStore.setState({ graphEntities: {} }));
  commitProjectLoadStep('history', () => useHistoryStore.setState(prepared.storeState.history));
  commitProjectLoadStep('project IO', () => useProjectIOStore.setState(prepared.storeState.projectIO));
  commitProjectLoadStep('completion log', () => {
    logger.sys.info('Project loaded (index from Rust)', 'ProjectIOStore');
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
      return commitPreparedAuthoritativeProjectLoad(prepared);
    } catch (err) {
      if (!isCurrentProjectIdentity(identity)) return null;
      const error = toErrorReference(err, PROJECT_LOAD_CONTRACT_ERROR_CODE);
      useProjectIOStore.setState({ status: LoadStatus.Error, error });
      logProjectIOError('Failed to load project', error);
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
  if (!projectIOApplicationPort().acceptProjectActivation(
    activation.projectInstanceId,
    activation.activationRevision,
  )) {
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

export const useProjectIOStore = create<ProjectIOStore>((set, _get) => ({
  status: LoadStatus.Idle,
  error: null,
  graphLoadStatus: {},

  currentPath: null,
  projectInstanceId: null,

  setCurrentPath: (path) => set({ currentPath: path ? formatDisplayPath(path) : null }),

  loadProject: async () => loadProjectForIdentity(captureProjectIdentity()),

  refreshResourceIndex: refreshProjectResourceIndex,

  loadProjectFromData: (project, path) => {
    projectIOApplicationPort().resetGraphProjection();
    loadGraphInFlight.clear();
    resetClientProjectState();
    set({ graphLoadStatus: {} });
    set({ projectInstanceId: null });
    useGraphDataStore.setState({ graphEntities: {} });
    const normalizedVariables = normalizeVariables(project.variables);
    const normalizedDatabases = applyDatabasesFromRaw(project.databases as Record<string, unknown>);
    useVariableStore.getState().setVariables(normalizedVariables);

    useResourceStore.getState().setSnapshot({
      resources: buildResourceIndex({
        graphs: Object.values(project.graphs).map((graph) => ({
          path: graph.path,
          name: graph.name,
          type: graph.type,
        })),
        worksheets: [],
        variables: normalizedVariables,
        databases: normalizedDatabases,
      }),
      graphOrder: Object.values(project.graphs).map((graph) => graph.path),
    });
    projectIOApplicationPort().reconcileOpenTabs();
    set({ status: LoadStatus.Ready, currentPath: path ? formatDisplayPath(path) : null });
  },

  loadGraph: async (graphPath) => {
    if (isGraphCachedInMemory(graphPath)) {
      set((state) => ({
        graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: 'ready' },
      }));
      return true;
    }

    const existing = loadGraphInFlight.get(graphPath);
    if (existing) return existing.promise;

    set((state) => ({
      graphLoadStatus: { ...state.graphLoadStatus, [graphPath]: 'loading' },
    }));
    const lifecycleToken = projectIOApplicationPort().beginGraphLoad(graphPath);
    const pending = projectIOApplicationPort().loadGraphProjection(graphPath, lifecycleToken)
      .catch((err) => {
        const error = toErrorReference(err, GRAPH_PROJECTION_CONTRACT_ERROR_CODE);
        set({ error });
        logProjectIOError('Failed to load graph projection', error);
        return false;
      })
      .then((loaded) => {
        const current = loadGraphInFlight.get(graphPath);
        if (current?.lifecycleToken === lifecycleToken && current.promise === pending) {
          set((state) => ({
            graphLoadStatus: {
              ...state.graphLoadStatus,
              [graphPath]: loaded ? 'ready' : 'error',
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
