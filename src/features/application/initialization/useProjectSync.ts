// src/features/application/initialization/useProjectSync.ts
// Application 层：协调 useEditor 与 Core 的 ProjectListener

import { useEffect, useRef, useMemo } from 'react';
import { ProjectListener } from '@/features/core/sync/listeners/ProjectListener';
import { SingletonManager } from '@/features/core/sync/utils/singletonManager';
import { useEditor } from '@/features/application/editor';
import type { EventCallbacks } from '@/features/core/sync/types';
import { logger } from '@/utils/appLogger';

const LISTENER_KEY = 'project-listener';

/**
 * 项目同步核心逻辑
 * 仅当 callbacks 存在时才更新监听器回调，避免 DataView 等非编辑器窗口覆盖 Editor 的回调
 */
function useProjectSyncCore(callbacks: EventCallbacks | undefined) {
  const isSetupRef = useRef(false);
  const listenerRef = useRef<ProjectListener | null>(null);

  useEffect(() => {
    if (isSetupRef.current) return;
    isSetupRef.current = true;

    const setup = async () => {
      const listener = await SingletonManager.getInstance(
        LISTENER_KEY,
        async () => {
          logger.sys.debug('Creating new listener instance', 'useProjectSync');
          const newListener = new ProjectListener(callbacks);
          await newListener.start();
          return newListener;
        }
      );
      listenerRef.current = listener;

      if (callbacks && SingletonManager.getRefCount(LISTENER_KEY) > 1) {
        listener.updateCallbacks(callbacks);
      }
    };

    setup();

    return () => {
      isSetupRef.current = false;
      listenerRef.current = null;
      SingletonManager.decrementRef(LISTENER_KEY, (instance: ProjectListener) => {
        instance.stop();
      });
    };
  }, []);

  // 当 callbacks 变化时更新（仅 useProjectSyncWithEditor 有此情况）
  useEffect(() => {
    if (callbacks && listenerRef.current) {
      listenerRef.current.updateCallbacks(callbacks);
    }
  }, [callbacks]);
}

/**
 * 带编辑器回调的项目同步（用于 EditorWindow）
 * Handlers 已直接更新 Store，callbacks 仅用于可选 UI 扩展（如打开新 Tab）
 */
export function useProjectSyncWithEditor() {
  const editor = useEditor({ withCanvasInteraction: false });
  const callbacks = useMemo<EventCallbacks>(
    () => ({
      onEventCreated: editor.handleEventCreated,
      onEventCreatedFailed: editor.handleEventCreatedFailed,
      onFunctionCreated: editor.handleFunctionCreated,
      onFunctionCreatedFailed: editor.handleFunctionCreatedFailed,
      onNodeCreated: editor.handleNodeCreated as EventCallbacks['onNodeCreated'],
      onNodeDeleted: editor.handleNodeDeleted,
    }),
    [
      editor.handleEventCreated,
      editor.handleEventCreatedFailed,
      editor.handleFunctionCreated,
      editor.handleFunctionCreatedFailed,
      editor.handleNodeCreated,
      editor.handleNodeDeleted,
    ]
  );
  useProjectSyncCore(callbacks);
}

/**
 * 无回调的项目同步（用于 DatabaseEditorWindow 等非编辑器窗口）
 * Handlers 已直接更新 Store，无需编辑器回调
 */
export function useProjectSync() {
  useProjectSyncCore(undefined);
}
