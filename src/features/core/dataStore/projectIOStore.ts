import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import type { ProjectData, Variable } from '@/shared/types';
import { ProjectService, toFrontendGraph } from '@/services/project/projectService';
import { GraphService } from '@/services/graph/graphService';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { logger } from '@/utils/appLogger';

import type { DatabaseRecord } from './databaseStore';
import { useVariableStore } from './variableStore';
import { useDatabaseStore } from './databaseStore';
import { useGraphDataStore } from './graphDataStore';
import { useEditStateStore } from './editStateStore';
import { useColumnStatsStore } from './columnStatsStore';
import { useColumnDistributionStore } from './columnDistributionStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useDatasetOverviewStore } from './datasetOverviewStore';
import { useHistoryStore } from '@/features/core/history';
import { getViewport, useViewportStore, ensureGraphViewport, syncGraphViewportsFromRecords } from '@/features/core/viewport';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { markResourceLoaded, resourceKey, useDocumentStateStore, useResourceStore, type ProjectResourceMeta } from '@/features/core/resource';
import {
  applySnapshotDocumentPatches,
  reconcileResourceSnapshot,
  type GraphFolderMeta,
} from '@/features/core/resource/resourceSnapshotReconcile';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { formatDisplayPath } from '@/shared/utils/formatDisplayPath';
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
} from '@/features/core/variable/variableCatalog';

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
  loadGraph(graphId: string): Promise<boolean>;
  exportSnapshot(): ProjectData;
}

/** 从 engine path 提取显示名称 */
function nameFromEngine(record: Record<string, unknown>): string | undefined {
  const engine = record?.engine as Record<string, unknown> | undefined;
  if (!engine) return undefined;
  const csv = engine.csv as { path?: string } | undefined;
  const parquet = engine.parquet as { path?: string } | undefined;
  const path = csv?.path ?? parquet?.path;
  if (typeof path === 'string') {
    const parts = path.replace(/\\/g, '/').split('/');
    const file = parts[parts.length - 1] || '';
    const stem = file.replace(/\.[^.]+$/, '');
    return stem || file || undefined;
  }
  return undefined;
}

/** 规范化数据库记录：补充 name（从 engine path 推导），合并已有富元数据 */
function normalizeDatabases(
  dbs: Record<string, DatabaseRecord>
): Record<string, DatabaseRecord> {
  const existing = useDatabaseStore.getState().databases;
  const result: Record<string, DatabaseRecord> = {};
  for (const [id, db] of Object.entries(dbs)) {
    const rec: Record<string, unknown> = typeof db === 'object' && db !== null ? { ...db } : { id };
    if (!rec.name) {
      rec.name = nameFromEngine(rec) ?? (existing[id] as Record<string, unknown>)?.name ?? id;
    }
    const prev = existing[id] as Record<string, unknown> | undefined;
    if (prev?.columns && !rec.columns) rec.columns = prev.columns;
    if (prev?.rowCount != null && rec.rowCount == null) rec.rowCount = prev.rowCount;
    if (prev?.columnCount != null && rec.columnCount == null) rec.columnCount = prev.columnCount;
    result[id] = rec as DatabaseRecord;
  }
  return result;
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
  graphs: Array<{ id: string; name: string; type: 'event' | 'function'; folderPath?: string }>;
  worksheets: Array<{ id: string; name: string; folderPath?: string }>;
  variables: Record<string, Variable>;
  databases: Record<string, DatabaseRecord>;
}): ProjectResourceMeta[] {
  const resources: ProjectResourceMeta[] = [];
  for (const graph of params.graphs) {
    resources.push({
      id: graph.id,
      kind: graph.type,
      name: graph.name,
      uri: `yssbi://graph/${graph.type}/${graph.id}`,
      folderPath: graph.folderPath,
      exists: true,
      loaded: Boolean(useGraphDataStore.getState().graphNodes[graph.id]),
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
  }
  for (const worksheet of params.worksheets) {
    resources.push({
      id: worksheet.id,
      kind: 'worksheet',
      name: worksheet.name,
      uri: `yssbi://worksheet/${worksheet.id}`,
      folderPath: worksheet.folderPath,
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

function normalizeGraphFolders(
  folders: Array<{ name: string; type: 'event' | 'function'; folderPath: string }> | undefined,
): GraphFolderMeta[] {
  return (folders ?? []).map((folder) => ({
    name: folder.name,
    type: folder.type,
    folderPath: folder.folderPath,
  }));
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

    const graphOrder = index.graphs.map((graph) => graph.id);
    const graphFolders = normalizeGraphFolders(index.folders);

    const worksheetIndex = (index.worksheets ?? []).map((ws) => ({
      id: ws.id,
      name: ws.name,
      databaseId: ws.databaseId,
      chartType: ws.chartType as import('@/shared/types/domain/worksheet').WorksheetChartType,
      folderPath: ws.folderPath ?? '',
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
      graphFolders,
      graphOrder,
    });
    return true;
  } catch (err) {
    const errorMessage = formatErrorMessage(err, 'Failed to refresh resource index');
    logger.sys.error('Failed to refresh resource index: ' + errorMessage, 'ProjectIOStore');
    useProjectIOStore.setState({ error: errorMessage });
    return false;
  }
}

function buildGraphSnapshot(): ProjectData['graphs'] {
  const resourceStore = useResourceStore.getState();
  const dataStore = useGraphDataStore.getState();

  return Object.fromEntries(
    resourceStore.graphOrder
      .map((graphId) => {
        const eventMeta = resourceStore.resources[resourceKey({ id: graphId, kind: 'event' })];
        const functionMeta = resourceStore.resources[resourceKey({ id: graphId, kind: 'function' })];
        const meta = eventMeta ?? functionMeta;
        if (!meta || !meta.exists) return null;

        const nodeIds = dataStore.graphNodes[graphId] ?? [];
        const nodes = nodeIds.map((nodeId) => dataStore.nodes[nodeId]).filter(Boolean);
        const pins = nodeIds.flatMap((nodeId) =>
          (dataStore.nodePins[nodeId] ?? []).map((pinId) => dataStore.pins[pinId]).filter(Boolean)
        );
        const connectionIds = new Set<string>();
        for (const pin of pins) {
          for (const connectionId of dataStore.pinConnections[pin.id] ?? []) {
            connectionIds.add(connectionId);
          }
        }
        const connections = Array.from(connectionIds)
          .map((connectionId) => dataStore.connections[connectionId])
          .filter(Boolean)
          .map((connection) => ({ fromPin: connection.from, toPin: connection.to }));

        return [
          graphId,
          {
            id: graphId,
            name: meta.name,
            type: meta.kind === 'function' ? 'function' : 'event',
            nodes,
            pins,
            connections: { connections },
            canvas: getViewport(graphId),
          },
        ] as [string, ProjectData['graphs'][string]];
      })
      .filter((entry): entry is [string, ProjectData['graphs'][string]] => entry !== null)
  ) as ProjectData['graphs'];
}

/**
 * Drop every per-project frontend cache: open graph tabs, viewports, history,
 * data-view caches. Called before hydrating a fresh project so the previous
 * project leaves no residue behind.
 *
 * Variables / databases / graphMeta / graphData are intentionally NOT cleared
 * here because the caller writes their replacement values immediately after.
 */
function resetClientProjectState(): void {
  useLayoutStore.getState().closeAllGraphTabs();
  useViewportStore.getState().clear();
  useHistoryStore.getState().clear();
  useEditStateStore.getState().clear();
  useColumnStatsStore.getState().clear();
  useColumnDistributionStore.getState().clear();
  useDatasetOverviewStore.getState().clear();
  useWorksheetStore.getState().clear();
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
}

/** 合并并发 load，避免 ProjectLoaded / 多窗口 / 初始化同时触发多路 get_project_* invoke */
let loadProjectInFlight: Promise<ProjectData | null> | null = null;

const loadGraphInFlight = new Map<string, Promise<boolean>>();

export const useProjectIOStore = create<ProjectIOStore>((set, get) => ({
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
        const normalizedDatabases = normalizeDatabases(databases as Record<string, DatabaseRecord>);

        const index = await ProjectService.getProjectIndex();
        const normalizedVariables = applyVariableCatalogFromIndex(index.variables);
        useVariableStore.getState().setVariables(normalizedVariables);
        useDatabaseStore.getState().setDatabases(normalizedDatabases);

        const graphOrder = index.graphs.map((graph) => graph.id);
        const worksheetIndex = (index.worksheets ?? []).map((ws) => ({
            id: ws.id,
            name: ws.name,
            databaseId: ws.databaseId,
            chartType: ws.chartType as import('@/shared/types/domain/worksheet').WorksheetChartType,
            folderPath: ws.folderPath ?? '',
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
          graphFolders: normalizeGraphFolders(index.folders),
          graphOrder,
        });

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
    const normalizedDatabases = normalizeDatabases(project.databases as unknown as Record<string, DatabaseRecord>);
    useVariableStore.getState().setVariables(normalizedVariables);
    useDatabaseStore.getState().setDatabases(normalizedDatabases);
    useGraphDataStore.getState().hydrateGraphs(project.graphs);
    useResourceStore.getState().setSnapshot({
      resources: buildResourceIndex({
        graphs: Object.values(project.graphs).map((graph) => ({
          id: graph.id,
          name: graph.name,
          type: graph.type,
        })),
        worksheets: [],
        variables: normalizedVariables,
        databases: normalizedDatabases,
      }),
      graphFolders: [],
      graphOrder: Object.keys(project.graphs),
    });
    syncGraphViewportsFromRecords(project.graphs);
    set({ status: LoadStatus.Ready, currentPath: path ? formatDisplayPath(path) : null });
  },

  loadGraph: async (graphId) => {
    const dataStore = useGraphDataStore.getState();
    if (dataStore.graphNodes[graphId]) return true;

    const existing = loadGraphInFlight.get(graphId);
    if (existing) return existing;

    const pending = (async (): Promise<boolean> => {
      try {
        const { graph, variables } = await ProjectService.loadProjectGraph(graphId);
        let frontendGraph;
        try {
          frontendGraph = await GraphService.resolveGraphDynamicPins(graphId);
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
        const resourceKeyValue = `graph:${frontendGraph.type}:${graphId}` as const;
        const existingResource = useResourceStore.getState().resources[resourceKeyValue];
        useResourceStore.getState().upsertResource({
          id: graphId,
          kind: frontendGraph.type,
          name: frontendGraph.name,
          uri: `yssbi://graph/${frontendGraph.type}/${graphId}`,
          folderPath: existingResource?.folderPath,
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        });
        markResourceLoaded({ id: graphId, kind: frontendGraph.type });
        useGraphDataStore.getState().addGraphFromData(graphId, frontendGraph);
        ensureGraphViewport(graphId, frontendGraph.canvas);
        return true;
      } catch (err) {
        const errorMessage = formatErrorMessage(err, 'Failed to load graph');
        logger.sys.error('Failed to load graph: ' + errorMessage, 'ProjectIOStore');
        set({ error: errorMessage });
        return false;
      } finally {
        loadGraphInFlight.delete(graphId);
      }
    })();

    loadGraphInFlight.set(graphId, pending);
    return pending;
  },

  exportSnapshot: () => {
    return {
      variables: useVariableStore.getState().variables,
      databases: useDatabaseStore.getState().databases,
      graphs: buildGraphSnapshot(),
      metadata: {
        exportTime: new Date().toISOString(),
        appVersion: '1.0.0',
      },
    } as unknown as ProjectData;
  },
}));
