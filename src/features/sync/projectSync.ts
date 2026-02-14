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
  const { 
    enabled = true, 
    onProjectLoaded, 
    onProjectCleared, 
    onProjectSaved,
    onEventCreated,
    onFunctionCreated,
    onMacroCreated,
  } = options;

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
        const eventData = event.payload;
        console.log(`[ProjectSync] Received event:`, eventData);

        const projectStore = useProjectStore.getState();

        // 处理嵌套的事件结构
        // 后端发送: Event::Event(EventEvent::EventCreated {...})
        // 序列化为: { type: "Event", payload: { type: "EventCreated", payload: {...} } }
        const eventType = eventData.type;
        const eventPayload = eventData.payload;

        // 如果是嵌套事件，提取内部类型
        let type: string;
        let payload: any;

        if (eventPayload && typeof eventPayload === 'object' && 'type' in eventPayload) {
          // 嵌套事件：Event/Function/Macro/Variable/Node/Connection/DataFrame
          type = eventPayload.type;
          payload = eventPayload.payload;
        } else {
          // 直接事件：Project
          type = eventType;
          payload = eventPayload;
        }

        console.log(`[ProjectSync] Processing event type: ${type}`);

        switch (type) {
          case 'ProjectLoaded':
            projectStore.loadProject(payload.data, payload.path);
            onProjectLoaded?.(payload.data, payload.path);
            break;

          case 'ProjectCleared':
            projectStore.loadProject(
              {
                variables: {},
                graphs: {},
                databases: {},
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
            console.log('[ProjectSync] EventCreated payload:', payload);
            console.log('[ProjectSync] payload.id:', payload.id);
            console.log('[ProjectSync] payload.data:', payload.data);
            // 使用新的 addGraph 方法
            projectStore.addGraph(payload.id, payload.data);
            console.log('[ProjectSync] Graphs after update:', projectStore.graphs);
            // 触发回调
            onEventCreated?.(payload.id, payload.data);
            break;

          case 'EventUpdated':
            // 使用新的 updateGraph 方法
            projectStore.updateGraph(payload.id, payload.data);
            break;

          case 'EventDeleted':
            // 使用新的 deleteGraph 方法
            projectStore.deleteGraph(payload.id);
            break;

          // Function 相关事件
          case 'FunctionCreated':
            projectStore.addGraph(payload.id, payload.data);
            // 触发回调
            onFunctionCreated?.(payload.id, payload.data);
            break;

          case 'FunctionUpdated':
            projectStore.updateGraph(payload.id, payload.data);
            break;

          case 'FunctionDeleted':
            projectStore.deleteGraph(payload.id);
            break;

          // Macro 相关事件
          case 'MacroCreated':
            projectStore.addGraph(payload.id, payload.data);
            // 触发回调
            onMacroCreated?.(payload.id, payload.data);
            break;

          case 'MacroUpdated':
            projectStore.updateGraph(payload.id, payload.data);
            break;

          case 'MacroDeleted':
            projectStore.deleteGraph(payload.id);
            break;

          // GlobalVariable 相关事件
          case 'GlobalVariableCreated':
            projectStore.addVariable(payload.id, payload.data);
            break;

          case 'GlobalVariableUpdated':
            projectStore.updateVariable(payload.id, payload.data);
            break;

          case 'GlobalVariableDeleted':
            projectStore.deleteVariable(payload.id);
            break;

          // DataFrame 相关事件
          case 'DataFrameCreated':
            projectStore.addDatabase(payload.id, payload.data);
            break;

          case 'DataFrameDeleted':
            projectStore.deleteDatabase(payload.id);
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
