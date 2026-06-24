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
import { useGraphMetaStore } from './graphMetaStore';
import { useGraphDataStore } from './graphDataStore';
import { useEditStateStore } from './editStateStore';
import { useColumnStatsStore } from './columnStatsStore';
import { useColumnDistributionStore } from './columnDistributionStore';
import { useDatasetOverviewStore } from './datasetOverviewStore';
import { useHistoryStore } from '@/features/core/history';
import { getViewport, useViewportStore } from '@/features/core/viewport';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';

interface ProjectIOStore {
  status: LoadStatus;
  error: string | null;

  currentPath: string | null;

  setCurrentPath(path: string | null): void;
  /** 从 Rust 当前项目状态拉取并灌入前端 store（路径、变量、库、图索引）；合并并发调用 */
  loadProject(): Promise<ProjectData | null>;
  loadProjectFromData(project: ProjectData, path: string | null): void;
  loadGraph(graphId: string): Promise<boolean>;
  exportSnapshot(): ProjectData;
}

/** 将 Graph 转为 GraphMeta 格式 */
function toGraphMetaMap(graphs: Record<string, { id: string; name: string; type: 'event' | 'function'; entryNodeId?: string }>): Record<string, { id: string; name: string; type: 'event' | 'function'; entryNodeId?: string }> {
  return Object.fromEntries(
    Object.entries(graphs).map(([id, g]) => [
      id,
      { id: g.id, name: g.name, type: g.type, entryNodeId: g.entryNodeId },
    ])
  );
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

function buildGraphSnapshot(): ProjectData['graphs'] {
  const metaStore = useGraphMetaStore.getState();
  const dataStore = useGraphDataStore.getState();

  return Object.fromEntries(
    metaStore.graphOrder
      .map((graphId) => {
        const meta = metaStore.graphs[graphId];
        if (!meta) return null;

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
            ...meta,
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
}

/** 合并并发 load，避免 ProjectLoaded / 多窗口 / 初始化同时触发多路 get_project_* invoke */
let loadProjectInFlight: Promise<ProjectData | null> | null = null;

const loadGraphInFlight = new Map<string, Promise<boolean>>();

export const useProjectIOStore = create<ProjectIOStore>((set, get) => ({
  status: LoadStatus.Idle,
  error: null,

  currentPath: null,

  setCurrentPath: (path) => set({ currentPath: path }),

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

        // 分阶段加载：先 databases + variables，再只加载图索引；图体在打开 Tab 时按需加载。
        const { databases, variables } = await ProjectService.getDatabasesVariables();
        useVariableStore.getState().setVariables(normalizeVariables(variables as Parameters<typeof normalizeVariables>[0]));
        useDatabaseStore.getState().setDatabases(normalizeDatabases(databases as Record<string, DatabaseRecord>));

        const index = await ProjectService.getProjectIndex();
        const graphMetaMap = Object.fromEntries(
          index.graphs.map((graph) => [
            graph.id,
            { id: graph.id, name: graph.name, type: graph.type, folderPath: graph.folderPath },
          ])
        );
        useGraphMetaStore.getState().setGraphs(graphMetaMap);
        useGraphMetaStore.getState().setGraphFolders(index.folders ?? []);
        useGraphDataStore.getState().hydrateGraphs({});

        set({ status: LoadStatus.Ready, currentPath: path });
        logger.sys.info('Project loaded (index from Rust)', 'ProjectIOStore');
        return {
          variables,
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

  loadProjectFromData: (project, path) => {
    resetClientProjectState();
    useVariableStore.getState().setVariables(normalizeVariables(project.variables));
    useDatabaseStore.getState().setDatabases(normalizeDatabases(project.databases as unknown as Record<string, DatabaseRecord>));
    useGraphMetaStore.getState().setGraphs(toGraphMetaMap(project.graphs));
    useGraphDataStore.getState().hydrateGraphs(project.graphs);
    set({ status: LoadStatus.Ready, currentPath: path });
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
        useGraphMetaStore.getState().updateGraph(graphId, {
          name: frontendGraph.name,
          type: frontendGraph.type,
        });
        useGraphDataStore.getState().addGraphFromData(graphId, frontendGraph);
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
