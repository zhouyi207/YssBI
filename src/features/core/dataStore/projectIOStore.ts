import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import type { ProjectData, Variable } from '@/shared/types';
import { ProjectService } from '@/services/project/projectService';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';

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
function toGraphMetaMap(graphs: Record<string, { id: string; name: string; type: 'event' | 'function' | 'macro'; entryNodeId?: string }>): Record<string, { id: string; name: string; type: 'event' | 'function' | 'macro'; entryNodeId?: string }> {
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
    const rec = typeof db === 'object' && db !== null ? { ...db } : { id };
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
      const projectData = await ProjectService.getProjectData();
      const path = await ProjectService.getProjectPath();

      // 分发到子 store（规范化变量 dataType）
      useVariableStore.getState().setVariables(normalizeVariables(projectData.variables));
      useDatabaseStore.getState().setDatabases(normalizeDatabases(projectData.databases as unknown as Record<string, DatabaseRecord>));
      useGraphMetaStore.getState().setGraphs(toGraphMetaMap(projectData.graphs));
      useGraphDataStore.getState().hydrateGraphs(projectData.graphs);

      set({ status: LoadStatus.Ready, currentPath: path });
      console.log('[ProjectIOStore] Project loaded successfully');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ status: LoadStatus.Error, error: errorMessage });
      console.error('[ProjectIOStore] Failed to load project:', errorMessage);
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
    if (get().status === LoadStatus.Loading) return null;

    set({ status: LoadStatus.Loading, error: null });

    try {
      const projectData = await ProjectService.getProjectState();
      const path = await ProjectService.getProjectPath();

      useVariableStore.getState().setVariables(normalizeVariables(projectData.variables));
      useDatabaseStore.getState().setDatabases(normalizeDatabases(projectData.databases as unknown as Record<string, DatabaseRecord>));
      useGraphMetaStore.getState().setGraphs(toGraphMetaMap(projectData.graphs));
      useGraphDataStore.getState().hydrateGraphs(projectData.graphs);
      useHistoryStore.getState().clear();

      set({ status: LoadStatus.Ready, currentPath: path });
      console.log('[ProjectIOStore] Synced from backend');
      return projectData;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ status: LoadStatus.Error, error: errorMessage });
      console.error('[ProjectIOStore] Failed to sync:', errorMessage);
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
