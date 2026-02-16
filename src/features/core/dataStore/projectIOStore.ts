import { create } from 'zustand';
import { LoadStatus } from '@/shared/types/ui/common';
import { ProjectData } from '@/shared/types';
import { ProjectService } from '@/services/project/projectService';

import { useVariableStore } from './variableStore';
import { useDatabaseStore } from './databaseStore';
import { useGraphMetaStore } from './graphMetaStore';
import { useGraphDataStore } from './graphDataStore';

interface ProjectIOStore {
  status: LoadStatus;
  error: string | null;

  currentPath: string | null;

  loadProject(): Promise<void>;
  exportSnapshot(): ProjectData;
}

export const useProjectIOStore = create<ProjectIOStore>((set, get) => ({
  status: LoadStatus.Idle,
  error: null,

  currentPath: null,

  loadProject: async () => {
    if (get().status === LoadStatus.Loading) return;

    set({ status: LoadStatus.Loading, error: null });

    try {
      const projectData: ProjectData = await ProjectService.getProjectData();

      // 分发到子 store
      useVariableStore.getState().setVariables(projectData.variables);
      useDatabaseStore.getState().setDatabases(projectData.databases);
      useGraphMetaStore.getState().setGraphs(projectData.graphs); // GraphMetaStore 只存图概览
      useGraphDataStore.getState().hydrateGraphs(projectData.graphs); // GraphDataStore 存节点/连接/pin

      set({ status: LoadStatus.Ready });
      console.log('[ProjectIOStore] Project loaded successfully');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ status: LoadStatus.Error, error: errorMessage });
      console.error('[ProjectIOStore] Failed to load project:', errorMessage);
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
