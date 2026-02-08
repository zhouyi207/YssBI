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

// 全局单例：确保事件监听器只注册一次
let globalUnlisten: (() => void) | null = null;
let listenerSetupPromise: Promise<void> | null = null;
let listenerCount = 0;

/**
 * 订阅后端项目事件，自动同步数据到前端 Store
 */
export function useProjectSync(options: UseProjectSyncOptions = {}) {
  const { enabled = true, onProjectLoaded, onProjectCleared, onProjectSaved } = options;

  useEffect(() => {
    if (!enabled) return;

    listenerCount++;

    // 如果已经有监听器，不需要再创建
    if (globalUnlisten) {
      console.log('[useProjectSync] Listener already exists, skipping setup');
      return () => {
        listenerCount--;
      };
    }

    // 如果正在设置监听器，等待完成
    if (listenerSetupPromise) {
      console.log('[useProjectSync] Listener setup in progress, waiting...');
      return () => {
        listenerCount--;
      };
    }

    const setupListener = async () => {
      console.log('[useProjectSync] Setting up project event listener...');

      globalUnlisten = await listen<ProjectEventPayload>('project-event', (event) => {
        const { type, payload } = event.payload;
        console.log(`[useProjectSync] Received event: type=${type}, payload=${JSON.stringify(payload)}`);

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
                dataframes: {},
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

          // DataFrame 相关事件
          case 'DataFrameCreated':
            projectStore.setDataFrames({ ...projectStore.dataframes, [payload.id]: payload.data });
            break;

          case 'DataFrameDeleted':
            const nextDataFrames = { ...projectStore.dataframes };
            delete nextDataFrames[payload.id];
            projectStore.setDataFrames(nextDataFrames);
            break;

          default:
            console.log(`[useProjectSync] Unhandled event type: ${type}`);
        }
      });

      console.log('[useProjectSync] Project event listener set up successfully');
    };

    listenerSetupPromise = setupListener().finally(() => {
      listenerSetupPromise = null;
    });

    return () => {
      listenerCount--;
      // 只有当没有其他组件使用时才清理监听器
      if (listenerCount === 0 && globalUnlisten) {
        globalUnlisten();
        globalUnlisten = null;
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

    console.log(`[initProjectSync] Received project data: events=${Object.keys(projectData.events || {}).length}, functions=${Object.keys(projectData.functions || {}).length}, macros=${Object.keys(projectData.macros || {}).length}, globalVars=${Object.keys(projectData.globalVariables || {}).length}, dataframes=${Object.keys(projectData.dataframes || {}).length}`);

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
