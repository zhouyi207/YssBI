import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { ProjectService } from '../../../services/projectService';
import { useProjectStore } from '../Store/useProjectStore';
import { ProjectData } from '../Types/canvas';

/**
 * 项目事件类型（与后端 ProjectEvent 对应）
 */
interface ProjectEventPayload {
  type: string;
  payload: any;
}

/**
 * useProjectSync 配置
 */
interface UseProjectSyncOptions {
  /** 是否启用同步 */
  enabled?: boolean;
  /** 项目加载回调 */
  onProjectLoaded?: (data: ProjectData, path: string | null) => void;
  /** 项目清除回调 */
  onProjectCleared?: () => void;
  /** 项目保存回调 */
  onProjectSaved?: (path: string) => void;
}

/**
 * 订阅后端项目事件，自动同步数据到前端 Store
 */
export function useProjectSync(options: UseProjectSyncOptions = {}) {
  const { enabled = true, onProjectLoaded, onProjectCleared, onProjectSaved } = options;

  useEffect(() => {
    if (!enabled) return;

    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      console.log('[useProjectSync] Setting up project event listener...');
      
      unlisten = await listen<ProjectEventPayload>('project-event', (event) => {
        const { type, payload } = event.payload;
        console.log('[useProjectSync] Received event:', type, payload);

        const projectStore = useProjectStore.getState();

        switch (type) {
          case 'ProjectLoaded':
            // 同步到前端 Store
            projectStore.loadProject(payload.data, payload.path);
            onProjectLoaded?.(payload.data, payload.path);
            break;

          case 'ProjectCleared':
            // 清空前端 Store
            projectStore.loadProject(
              {
                globalVariables: {},
                events: {},
                functions: {},
                macros: {},
                metadata: { exportTime: '', appVersion: '' },
              },
              null
            );
            onProjectCleared?.();
            break;

          case 'ProjectSaved':
            projectStore.setCurrentPath(payload.path);
            onProjectSaved?.(payload.path);
            break;

          // Event 相关事件
          case 'EventCreated':
            projectStore.setEvents({ ...projectStore.events, [payload.id]: payload.data });
            break;

          case 'EventUpdated':
            if (projectStore.events[payload.id]) {
              projectStore.setEvents({ ...projectStore.events, [payload.id]: payload.data });
            }
            break;

          case 'EventDeleted':
            const nextEvents = { ...projectStore.events };
            delete nextEvents[payload.id];
            projectStore.setEvents(nextEvents);
            break;

          // Function 相关事件
          case 'FunctionCreated':
            projectStore.setFunctions({ ...projectStore.functions, [payload.id]: payload.data });
            break;

          case 'FunctionUpdated':
            if (projectStore.functions[payload.id]) {
              projectStore.setFunctions({ ...projectStore.functions, [payload.id]: payload.data });
            }
            break;

          case 'FunctionDeleted':
            const nextFunctions = { ...projectStore.functions };
            delete nextFunctions[payload.id];
            projectStore.setFunctions(nextFunctions);
            break;

          // Macro 相关事件
          case 'MacroCreated':
            projectStore.setMacros({ ...projectStore.macros, [payload.id]: payload.data });
            break;

          case 'MacroUpdated':
            if (projectStore.macros[payload.id]) {
              projectStore.setMacros({ ...projectStore.macros, [payload.id]: payload.data });
            }
            break;

          case 'MacroDeleted':
            const nextMacros = { ...projectStore.macros };
            delete nextMacros[payload.id];
            projectStore.setMacros(nextMacros);
            break;

          // GlobalVariable 相关事件
          case 'GlobalVariableCreated':
            projectStore.setGlobalVariables({ ...projectStore.globalVariables, [payload.id]: payload.data });
            break;

          case 'GlobalVariableUpdated':
            if (projectStore.globalVariables[payload.id]) {
              projectStore.setGlobalVariables({ ...projectStore.globalVariables, [payload.id]: payload.data });
            }
            break;

          case 'GlobalVariableDeleted':
            const nextGlobalVariables = { ...projectStore.globalVariables };
            delete nextGlobalVariables[payload.id];
            projectStore.setGlobalVariables(nextGlobalVariables);
            break;

          default:
            console.log('[useProjectSync] Unhandled event type:', type);
        }
      });

      console.log('[useProjectSync] Project event listener set up successfully');
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
        console.log('[useProjectSync] Project event listener cleaned up');
      }
    };
  }, [enabled, onProjectLoaded, onProjectCleared, onProjectSaved]);
}

/**
 * 初始化时从后端同步项目状态
 * 应该在应用启动时调用一次
 */
export async function initProjectSync(): Promise<ProjectData | null> {
  try {
    console.log('[initProjectSync] Syncing project state from backend...');
    const projectData = await ProjectService.getProjectState();
    
    console.log('[initProjectSync] Received project data:', {
      eventsCount: Object.keys(projectData.events || {}).length,
      functionsCount: Object.keys(projectData.functions || {}).length,
      macrosCount: Object.keys(projectData.macros || {}).length,
      globalVariablesCount: Object.keys(projectData.globalVariables || {}).length,
    });

    // 同步到前端 Store
    const projectStore = useProjectStore.getState();
    const path = await ProjectService.getProjectPath();
    projectStore.loadProject(projectData, path);

    console.log('[initProjectSync] Project state synced successfully');
    return projectData;
  } catch (error) {
    console.error('[initProjectSync] Failed to sync project state:', error);
    return null;
  }
}
