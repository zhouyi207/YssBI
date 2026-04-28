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

interface ProjectIOStore {
  status: LoadStatus;
  error: string | null;

  currentPath: string | null;

  setCurrentPath(path: string | null): void;
  loadProject(): Promise<void>;
  loadProjectFromData(project: ProjectData, path: string | null): void;
  syncFromBackend(): Promise<ProjectData | null>;
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

      // 分阶段加载：先 databases + variables，再 graphs（根据前者校验引用）
      const { databases, variables } = await ProjectService.getDatabasesVariables();
      useVariableStore.getState().setVariables(normalizeVariables(variables as Parameters<typeof normalizeVariables>[0]));
      useDatabaseStore.getState().setDatabases(normalizeDatabases(databases as Record<string, DatabaseRecord>));

      const { graphs, invalidReferences } = await ProjectService.getProjectGraphs();
      const graphMap = Object.fromEntries(
        Object.entries(graphs).map(([id, dto]) => [id, toFrontendGraph(dto)])
      );
      useGraphMetaStore.getState().setGraphs(toGraphMetaMap(graphMap));
      useGraphDataStore.getState().hydrateGraphs(graphMap);
      useHistoryStore.getState().clear();

      const invalidCount = Object.values(invalidReferences).reduce((s, arr) => s + arr.length, 0);
      if (invalidCount > 0) {
        logger.sys.warn(`发现无效引用: ${JSON.stringify(invalidReferences)}, 共 ${invalidCount} 处`, 'ProjectIOStore');
      }

      set({ status: LoadStatus.Ready, currentPath: path });
      logger.sys.debug('Synced from backend (staged load)', 'ProjectIOStore');
      return {
        variables,
        databases,
        graphs: graphMap,
        metadata: { exportTime: '', appVersion: '' },
      } as ProjectData;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ status: LoadStatus.Error, error: errorMessage });
      logger.sys.error('Failed to sync: ' + errorMessage, 'ProjectIOStore');
      return null;
    }
  },

  exportSnapshot: () => {
    return {
      variables: useVariableStore.getState().variables,
      databases: useDatabaseStore.getState().databases,
      graphs: useGraphMetaStore.getState().graphs, // 图概览
      metadata: {
        exportTime: new Date().toISOString(),
        appVersion: '1.0.0',
      },
    } as unknown as ProjectData;
  },
}));
