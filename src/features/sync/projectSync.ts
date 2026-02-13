/// hooks —— 生命周期 + 组合逻辑（重点）

import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useProjectStore } from '../project';
import { UseProjectSyncOptions, ProjectEventPayload } from '../project';

// 全局单例：确保事件监听器只注册一次
let globalUnlisten: (() => void) | null = null;
let listenerSetupPromise: Promise<void> | null = null;
let listenerCount = 0;


/**
 * 订阅后端项目事件，自动同步数据到前端 Store
 *
 * 语义：
 * - 全局单例模式，确保事件监听器只注册一次
 * - 自动处理后端推送的项目变更事件
 */
export function useProjectSync(options: UseProjectSyncOptions = {}) {
  const { enabled = true, onProjectLoaded, onProjectCleared, onProjectSaved } = options;

  useEffect(() => {
    if (!enabled) return;

    listenerCount++;

    // 如果已经有监听器，不需要再创建
    if (globalUnlisten) {
      console.log('[ProjectSync] Listener already exists, reusing...');
      return () => {
        listenerCount--;
      };
    }

    // 如果正在设置监听器，等待完成
    if (listenerSetupPromise) {
      console.log('[ProjectSync] Listener setup in progress, waiting...');
      return () => {
        listenerCount--;
      };
    }

    const setupListener = async () => {
      console.log('[ProjectSync] Setting up project event listener...');

      globalUnlisten = await listen<ProjectEventPayload>('project-event', (event) => {
        const { type, payload } = event.payload;
        console.log(`[ProjectSync] Received event: ${type}`);

        const projectStore = useProjectStore.getState();

        switch (type) {
          case 'ProjectLoaded':
            projectStore.loadProject(payload.data, payload.path);
            onProjectLoaded?.(payload.data, payload.path);
            break;

          case 'ProjectCleared':
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
            console.log(`[ProjectSync] Unhandled event type: ${type}`);
        }
      });

      console.log('[ProjectSync] ✓ Project event listener set up successfully');
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
        console.log('[ProjectSync] Project event listener cleaned up');
      }
    };
  }, [enabled, onProjectLoaded, onProjectCleared, onProjectSaved]);
}
