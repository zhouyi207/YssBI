import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import type { ProjectData, Variable } from '@/shared/types';
import { ProjectService, toFrontendGraph } from '@/services/project/projectService';
import { GraphService } from '@/services/graph/graphService';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { logger } from '@/utils/appLogger';

import type { DatabaseRecord } from '@/shared/types/dto/database';
import { normalizeDatabases } from '@/shared/types/dto/database';
import { domainGraphRecordToGraphData, graphDataRecordToDomainGraphs } from '@/shared/types/dto/graphModel';
import { useVariableStore } from './variableStore';
import { useDatabaseStore } from './databaseStore';
import { useGraphDataStore } from './graphDataStore';
import { syncFunctionSignatureFromGraph, hydrateFunctionSignaturesFromProjectIndex } from '@/features/application/graphDocument/functionSignatureSync';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { ensureGraphViewport, syncGraphViewportsFromRecords } from '@/features/core/viewport';
import { buildGraphResourceMeta, markResourceLoaded, useResourceStore, type ProjectResourceMeta } from '@/features/core/resource';
import {
  applySnapshotDocumentPatches,
  reconcileResourceSnapshot,
} from '@/features/core/resource/resourceSnapshotReconcile';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { formatDisplayPath } from '@/shared/utils/formatDisplayPath';
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
} from '@/features/core/variable/variableCatalog';
import { resetClientProjectState } from './projectClientReset';
import { buildGraphSnapshotFromStores } from './projectSnapshotBridge';
import { reconcileOpenLayoutTabsWithResources } from '@/features/application/editor/reconcileOpenLayoutTabs';

interface ProjectIOStore {
  status: LoadStatus;
  error: string | null;

  currentPath: string | null;

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
  graphs: Array<{ path: string; name: string; type: 'event' | 'function' }>;
  worksheets: Array<{ id: string; name: string; databaseId: string; chartType: import('@/shared/types/domain/worksheet').WorksheetChartType }>;
  variables: Record<string, Variable>;
  databases: Record<string, DatabaseRecord>;
}): ProjectResourceMeta[] {
  const resources: ProjectResourceMeta[] = [];
  for (const graph of params.graphs) {
    resources.push(buildGraphResourceMeta(graph.type, graph.path, graph.name));
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
  try {
    const index = await ProjectService.getProjectIndex();
    const variableCatalog = applyVariableCatalogFromIndex(index.variables);
    useVariableStore.getState().setVariables(variableCatalog);

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
    const errorMessage = formatErrorMessage(err, 'Failed to refresh resource index');
    logger.sys.error('Failed to refresh resource index: ' + errorMessage, 'ProjectIOStore');
    useProjectIOStore.setState({ error: errorMessage });
    return false;
  }
}

/** 将后端/快照 databases 规范化并写入 store（合并已有富元数据） */
function applyDatabasesFromRaw(raw: Record<string, unknown>): Record<string, DatabaseRecord> {
  const normalized = normalizeDatabases(raw, useDatabaseStore.getState().databases);
  useDatabaseStore.getState().setDatabases(normalized);
  return normalized;
}

/** 合并并发 load，避免 ProjectLoaded / 多窗口 / 初始化同时触发多路 get_project_* invoke */
let loadProjectInFlight: Promise<ProjectData | null> | null = null;

const loadGraphInFlight = new Map<string, Promise<boolean>>();

export const useProjectIOStore = create<ProjectIOStore>((set, _get) => ({
  status: LoadStatus.Idle,
  error: null,

  currentPath: null,

  setCurrentPath: (path) => set({ currentPath: path ? formatDisplayPath(path) : null }),

  loadProject: async () => {
    if (loadProjectInFlight) {
      return loadProjectInFlight;
    }

    // 不因 Loading 提前返回，避免与 ProjectLoaded 事件 handler 竞态导致 importGraph 误判失败
    loadProjectInFlight = (async () => {
      set({ status: LoadStatus.Loading, error: null });

      try {
        const path = await ProjectService.getProjectPath();

        // Drop residue from any previously loaded project before hydrating new
        // values. Backend authoritatively cleared its in-memory state already.
        resetClientProjectState();

        const { databases } = await ProjectService.getDatabasesVariables();
        const normalizedDatabases = applyDatabasesFromRaw(databases as Record<string, unknown>);

        const index = await ProjectService.getProjectIndex();
        const normalizedVariables = applyVariableCatalogFromIndex(index.variables);
        useVariableStore.getState().setVariables(normalizedVariables);

        const graphOrder = index.graphs.map((graph) => graph.path);
        const worksheetIndex = (index.worksheets ?? []).map((ws) => ({
            id: ws.id,
            name: ws.name,
            databaseId: ws.databaseId,
            chartType: ws.chartType as import('@/shared/types/domain/worksheet').WorksheetChartType,
          }));
        useWorksheetStore.getState().setIndex(
          worksheetIndex,
        );
        useGraphDataStore.getState().hydrateGraphs({});
        useResourceStore.getState().setSnapshot({
          resources: buildResourceIndex({
            graphs: index.graphs,
            worksheets: worksheetIndex,
            variables: normalizedVariables,
            databases: normalizedDatabases,
          }),
          graphOrder,
        });
        hydrateFunctionSignaturesFromProjectIndex(index.graphs);
        reconcileOpenLayoutTabsWithResources();

        set({ status: LoadStatus.Ready, currentPath: path ? formatDisplayPath(path) : null });
        logger.sys.info('Project loaded (index from Rust)', 'ProjectIOStore');
        return {
          variables: normalizedVariables,
          databases,
          graphs: {},
          metadata: { exportTime: index.exportTime, appVersion: index.appVersion },
        } as ProjectData;
      } catch (err) {
        const errorMessage = formatErrorMessage(err, 'Failed to load project');
        set({ status: LoadStatus.Error, error: errorMessage });
        logger.sys.error('Failed to load project: ' + errorMessage, 'ProjectIOStore');
        return null;
      } finally {
        loadProjectInFlight = null;
      }
    })();

    return loadProjectInFlight;
  },

  refreshResourceIndex: refreshProjectResourceIndex,

  loadProjectFromData: (project, path) => {
    resetClientProjectState();
    const normalizedVariables = normalizeVariables(project.variables);
    const normalizedDatabases = applyDatabasesFromRaw(project.databases as Record<string, unknown>);
    useVariableStore.getState().setVariables(normalizedVariables);
    useGraphDataStore.getState().hydrateGraphs(domainGraphRecordToGraphData(project.graphs));
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
    syncGraphViewportsFromRecords(project.graphs);
    reconcileOpenLayoutTabsWithResources();
    set({ status: LoadStatus.Ready, currentPath: path ? formatDisplayPath(path) : null });
  },

  loadGraph: async (graphPath) => {
    const existing = loadGraphInFlight.get(graphPath);
    if (existing) return existing;

    const pending = (async (): Promise<boolean> => {
      try {
        const { graph, variables } = await ProjectService.loadProjectGraph(graphPath);
        let frontendGraph;
        try {
          frontendGraph = await GraphService.resolveGraphDynamicPins(graphPath);
        } catch (resolveErr) {
          logger.sys.warn(
            'Dynamic pin materialize failed, using loaded graph: ' +
              formatErrorMessage(resolveErr, 'resolve failed'),
            'ProjectIOStore',
          );
          frontendGraph = toFrontendGraph(graph);
        }
        useVariableStore.getState().setVariables({
          ...useVariableStore.getState().variables,
          ...normalizeVariables(variables as Parameters<typeof normalizeVariables>[0]),
        });
        useResourceStore.getState().upsertResource(
          buildGraphResourceMeta(frontendGraph.type, graphPath, frontendGraph.name),
        );
        markResourceLoaded({ id: graphPath, kind: frontendGraph.type });
        syncFunctionSignatureFromGraph({
          ...frontendGraph,
        });
        useGraphDataStore.getState().addGraphFromData(graphPath, frontendGraph);
        ensureGraphViewport(graphPath, frontendGraph.canvas);
        return true;
      } catch (err) {
        const errorMessage = formatErrorMessage(err, 'Failed to load graph');
        logger.sys.error('Failed to load graph: ' + errorMessage, 'ProjectIOStore');
        set({ error: errorMessage });
        return false;
      } finally {
        loadGraphInFlight.delete(graphPath);
      }
    })();

    loadGraphInFlight.set(graphPath, pending);
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
