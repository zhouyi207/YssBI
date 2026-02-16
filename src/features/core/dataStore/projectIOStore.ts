import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import { ProjectData } from '@/shared/types';
import { ProjectService } from '@/services/project/projectService';

import { useVariableStore } from './variableStore';
import { useDatabaseStore } from './databaseStore';
import { useGraphMetaStore } from './graphMetaStore';
import { useGraphDataStore } from './graphDataStore';
import { useGraphHistoryStore } from './graphHistoryStore';

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
function toGraphMetaMap(graphs: Record<string, any>): Record<string, { id: string; name: string; type: 'event' | 'function' | 'macro'; entryNodeId?: string }> {
  return Object.fromEntries(
    Object.entries(graphs).map(([id, g]) => [
      id,
      { id: g.id, name: g.name, type: g.type, entryNodeId: g.entryNodeId },
    ])
  );
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

      // 分发到子 store
      useVariableStore.getState().setVariables(projectData.variables);
      useDatabaseStore.getState().setDatabases(projectData.databases);
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
    useVariableStore.getState().setVariables(project.variables);
    useDatabaseStore.getState().setDatabases(project.databases);
    useGraphMetaStore.getState().setGraphs(toGraphMetaMap(project.graphs));
    useGraphDataStore.getState().hydrateGraphs(project.graphs);
    useGraphHistoryStore.getState().clearAll();
    set({ status: LoadStatus.Ready, currentPath: path });
  },

  syncFromBackend: async () => {
    if (get().status === LoadStatus.Loading) return null;

    set({ status: LoadStatus.Loading, error: null });

    try {
      const projectData = await ProjectService.getProjectState();
      const path = await ProjectService.getProjectPath();

      useVariableStore.getState().setVariables(projectData.variables);
      useDatabaseStore.getState().setDatabases(projectData.databases);
      useGraphMetaStore.getState().setGraphs(toGraphMetaMap(projectData.graphs));
      useGraphDataStore.getState().hydrateGraphs(projectData.graphs);
      useGraphHistoryStore.getState().clearAll();

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
    } as ProjectData;
  },
}));
