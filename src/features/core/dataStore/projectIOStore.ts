import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import type { ProjectData, Variable } from '@/shared/types';
import {
  ProjectService,
  type ProjectActivationResult,
} from '@/services/project/projectService';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { logger } from '@/utils/appLogger';

import type { DatabaseRecord } from '@/shared/types/dto/database';
import { normalizeDatabases } from '@/shared/types/dto/database';
import { graphDataRecordToDomainGraphs } from '@/shared/types/dto/graphModel';
import { useVariableStore } from './variableStore';
import { useDatabaseStore } from './databaseStore';
import { useGraphDataStore } from './graphDataStore';

import { hydrateFunctionSignaturesFromProjectIndex } from '@/features/application/graphDocument/functionSignatureSync';
import { resetFunctionSignatureCoordinator } from '@/features/application/editorMutation/functionSignatureCoordinator';
import { resetHistoryCoordinator } from '@/features/application/editorMutation/historyCoordinator';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { buildGraphResourceMeta, useResourceStore, type ProjectResourceMeta } from '@/features/core/resource';
import {
  applySnapshotDocumentPatches,
  reconcileResourceSnapshot,
} from '@/features/core/resource/resourceSnapshotReconcile';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { formatDisplayPath } from '@/shared/utils/formatDisplayPath';
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableRevisionsFromIndex,
} from '@/features/core/variable/variableCatalog';
import { resetClientProjectState } from './projectClientReset';
import { useGraphMetaStore } from './graphMetaStore';
import { useDocumentStateStore } from '@/features/core/resource/documentStateStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useViewportStore } from '@/features/core/viewport';
import { useGraphInteractionStore } from '@/features/core/graphInteraction';
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
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { useHistoryStore } from '@/features/core/history';
import { buildGraphSnapshotFromStores } from './projectSnapshotBridge';
import { isGraphCachedInMemory } from './graphDocumentLoadPolicy';
import { reconcileOpenLayoutTabsWithResources } from '@/features/application/editor/reconcileOpenLayoutTabs';
import {
  beginGraphLoadLifecycle,
  loadGraphProjection,
  resetGraphProjectionCoordinator,
} from '@/features/application/editorProjection/graphProjectionCoordinator';

interface ProjectIOStore {
  status: LoadStatus;
  error: string | null;

  currentPath: string | null;
  projectInstanceId: string | null;

  setCurrentPath(path: string | null): void;
  /** 从 Rust 当前项目状态拉取并灌入前端 store（路径、变量、库、图索引）；合并并发调用 */
  loadProject(): Promise<ProjectData | null>;
  /** 只刷新资源索引，不重置 tab、viewport、history 或已加载 graph 正文。 */
  refreshResourceIndex(): Promise<boolean>;
  loadProjectFromData(project: ProjectData, path: string | null): void;
  loadGraph(graphPath: string): Promise<boolean>;
  exportSnapshot(): ProjectData;
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
  worksheets: Array<{ id: string; name: string; databaseId: string; chartType: import('@/shared/types/domain/worksheet').WorksheetChartType }>;
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
      id: worksheet.id,
      kind: 'worksheet',
      name: worksheet.name,
      uri: `yssbi://worksheet/${worksheet.id}`,
      exists: true,
      loaded: Boolean(useWorksheetStore.getState().documents[worksheet.id]),
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

    const databaseRows = index.databases ?? [];
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

    const worksheetIndex = (index.worksheets ?? []).map((ws) => ({
      id: ws.id,
      name: ws.name,
      databaseId: ws.databaseId,
      chartType: ws.chartType as import('@/shared/types/domain/worksheet').WorksheetChartType,
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
    hydrateFunctionSignaturesFromProjectIndex(index.graphs);
    reconcileOpenLayoutTabsWithResources();
    return true;
  } catch (err) {
    if (!isCurrentProjectIdentity(identity)) return false;
    const errorMessage = formatErrorMessage(err, 'Failed to refresh resource index');
    logger.sys.error('Failed to refresh resource index: ' + errorMessage, 'ProjectIOStore');
    useProjectIOStore.setState({ error: errorMessage });
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
      layoutNodes: useLayoutStore.getState().nodes,
      editorTabs: useEditorTabStore.getState().snapshotMemento(),
      recentEditorGroupIds: useLayoutStore.getState().recentEditorGroupIds,
    },
    {
      ...defaultAuthoritativeProjectLoadPlanDependencies,
      validateCoordinatorStart: (projectInstanceId, publicationRevision) => {
        projectPublicationCoordinator.validateProjectStart(projectInstanceId, publicationRevision);
      },
      ...dependencyOverrides,
    },
  );
}

function commitProjectLoadStep(label: string, assignment: () => void): void {
  try {
    assignment();
  } catch (error) {
    try {
      logger.sys.error(
        `Project load commit listener failed at '${label}': ${formatErrorMessage(error)}`,
        'ProjectIOStore',
      );
    } catch {
      // Commit completion must not depend on diagnostics infrastructure.
    }
  }
}

export function commitPreparedAuthoritativeProjectLoad(
  prepared: PreparedAuthoritativeProjectLoad,
): ProjectData {
  projectPublicationCoordinator.startProject(
    prepared.index.projectInstanceId,
    prepared.index.publicationRevision,
  );
  commitProjectLoadStep('graph projection coordinator', resetGraphProjectionCoordinator);
  loadGraphInFlight.clear();
  commitProjectLoadStep('function signature coordinator', resetFunctionSignatureCoordinator);
  commitProjectLoadStep('history coordinator', resetHistoryCoordinator);

  commitProjectLoadStep('layout', () => useLayoutStore.setState({
    nodes: prepared.storeState.layout.nodes,
    activeEditorGroupId: prepared.storeState.layout.activeEditorGroupId,
    recentEditorGroupIds: prepared.storeState.layout.recentEditorGroupIds,
  }));
  commitProjectLoadStep('editor tabs', () => useEditorTabStore.setState(
    prepared.storeState.layout.tabs,
  ));
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
      const errorMessage = formatErrorMessage(err, 'Failed to load project');
      useProjectIOStore.setState({ status: LoadStatus.Error, error: errorMessage });
      logger.sys.error('Failed to load project: ' + errorMessage, 'ProjectIOStore');
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
  if (!projectPublicationCoordinator.acceptProjectActivation(
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

  currentPath: null,
  projectInstanceId: null,

  setCurrentPath: (path) => set({ currentPath: path ? formatDisplayPath(path) : null }),

  loadProject: async () => loadProjectForIdentity(captureProjectIdentity()),

  refreshResourceIndex: refreshProjectResourceIndex,

  loadProjectFromData: (project, path) => {
    resetGraphProjectionCoordinator();
    loadGraphInFlight.clear();
    resetClientProjectState();
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
    reconcileOpenLayoutTabsWithResources();
    set({ status: LoadStatus.Ready, currentPath: path ? formatDisplayPath(path) : null });
  },

  loadGraph: async (graphPath) => {
    if (isGraphCachedInMemory(graphPath)) {
      return true;
    }

    const existing = loadGraphInFlight.get(graphPath);
    if (existing) return existing.promise;

    const lifecycleToken = beginGraphLoadLifecycle(graphPath);
    const pending = loadGraphProjection(graphPath, lifecycleToken)
      .catch((err) => {
        const errorMessage = formatErrorMessage(err, 'Failed to load graph projection');
        logger.sys.error('Failed to load graph projection: ' + errorMessage, 'ProjectIOStore');
        set({ error: errorMessage });
        return false;
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

  exportSnapshot: (): ProjectData => ({
    variables: useVariableStore.getState().variables,
    databases: useDatabaseStore.getState().databases,
    graphs: graphDataRecordToDomainGraphs(buildGraphSnapshotFromStores()),
    metadata: {
      exportTime: new Date().toISOString(),
      appVersion: '1.0.0',
    },
  }),
}));
