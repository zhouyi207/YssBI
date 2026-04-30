import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import type { ProjectData, Variable } from '@/shared/types';
import { ProjectService, toFrontendGraph } from '@/services/project/projectService';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';
import { logger } from '@/utils/appLogger';

import type { DatabaseRecord } from './databaseStore';
import { useVariableStore } from './variableStore';
import { useDatabaseStore } from './databaseStore';
import { useGraphMetaStore } from './graphMetaStore';
import { useGraphDataStore } from './graphDataStore';
import { useHistoryStore } from '@/features/core/history';
import { getViewport } from '@/features/core/viewport';

interface ProjectIOStore {
  status: LoadStatus;
  error: string | null;

  currentPath: string | null;

  setCurrentPath(path: string | null): void;
  loadProject(): Promise<void>;
  loadProjectFromData(project: ProjectData, path: string | null): void;
  syncFromBackend(): Promise<ProjectData | null>;
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

export const useProjectIOStore = create<ProjectIOStore>((set, get) => ({
  status: LoadStatus.Idle,
  error: null,

  currentPath: null,

  setCurrentPath: (path) => set({ currentPath: path }),

  loadProject: async () => {
    if (get().status === LoadStatus.Loading) return;

    set({ status: LoadStatus.Loading, error: null });

    try {
      const result = await get().syncFromBackend();
      if (result) {
        logger.sys.info('Project loaded successfully', 'ProjectIOStore');
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ status: LoadStatus.Error, error: errorMessage });
      logger.sys.error('Failed to load project: ' + errorMessage, 'ProjectIOStore');
    }
  },

  loadProjectFromData: (project, path) => {
    useVariableStore.getState().setVariables(normalizeVariables(project.variables));
    useDatabaseStore.getState().setDatabases(normalizeDatabases(project.databases as unknown as Record<string, DatabaseRecord>));
    useGraphMetaStore.getState().setGraphs(toGraphMetaMap(project.graphs));
    useGraphDataStore.getState().hydrateGraphs(project.graphs);
    useHistoryStore.getState().clear();
    set({ status: LoadStatus.Ready, currentPath: path });
  },

  syncFromBackend: async () => {
    // 不因 Loading 提前返回，避免与 ProjectLoaded 事件 handler 竞态导致 importGraph 误判失败
    set({ status: LoadStatus.Loading, error: null });

    try {
      const path = await ProjectService.getProjectPath();

      // 分阶段加载：先 databases + variables，再只加载图索引；图体在打开 Tab 时按需加载。
      const { databases, variables } = await ProjectService.getDatabasesVariables();
      useVariableStore.getState().setVariables(normalizeVariables(variables as Parameters<typeof normalizeVariables>[0]));
      useDatabaseStore.getState().setDatabases(normalizeDatabases(databases as Record<string, DatabaseRecord>));

      const index = await ProjectService.getProjectIndex();
      const graphMetaMap = Object.fromEntries(
        index.graphs.map((graph) => [
          graph.id,
          { id: graph.id, name: graph.name, type: graph.type },
        ])
      );
      useGraphMetaStore.getState().setGraphs(graphMetaMap);
      useGraphDataStore.getState().hydrateGraphs({});
      useHistoryStore.getState().clear();

      set({ status: LoadStatus.Ready, currentPath: path });
      logger.sys.debug('Synced from backend (project index load)', 'ProjectIOStore');
      return {
        variables,
        databases,
        graphs: {},
        metadata: { exportTime: index.exportTime, appVersion: index.appVersion },
      } as ProjectData;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ status: LoadStatus.Error, error: errorMessage });
      logger.sys.error('Failed to sync: ' + errorMessage, 'ProjectIOStore');
      return null;
    }
  },

  loadGraph: async (graphId) => {
    const dataStore = useGraphDataStore.getState();
    if (dataStore.graphNodes[graphId]) return true;
    try {
      const { graph, variables } = await ProjectService.loadProjectGraph(graphId);
      const frontendGraph = toFrontendGraph(graph);
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
      const errorMessage = err instanceof Error ? err.message : String(err);
      logger.sys.error('Failed to load graph: ' + errorMessage, 'ProjectIOStore');
      set({ error: errorMessage });
      return false;
    }
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
