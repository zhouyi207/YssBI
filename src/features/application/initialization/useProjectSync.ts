// src/features/application/initialization/useProjectSync.ts
// Application 层：协调 useEditor 与 Core 的 ProjectListener

import { useEffect, useRef } from 'react';
import { initProjectSync } from '@/features/core/dataStore';
import { ProjectListener } from '@/features/core/sync/listeners/ProjectListener';
import { SingletonManager } from '@/features/core/sync/utils/singletonManager';
import { logger } from '@/utils/appLogger';

const LISTENER_KEY = 'project-listener';

/**
 * 项目同步核心逻辑
 * 仅当 callbacks 存在时才更新监听器回调，避免 DataView 等非编辑器窗口覆盖 Editor 的回调
 */
function useProjectSyncCore(callbacks?: import('@/features/core/sync/types').EventCallbacks) {
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
        },
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

  useEffect(() => {
    if (callbacks && listenerRef.current) {
      listenerRef.current.updateCallbacks(callbacks);
    }
  }, [callbacks]);
}

/**
 * 项目事件同步（EditorWindow / DatabaseEditorWindow 等共用）
 * Store 由 Core handlers 更新；graph 创建走 file-first，由 resourceActions 刷新索引。
 */
export function useProjectSync() {
  useEffect(() => {
    void initProjectSync().catch((error) => {
      logger.sys.error(`Failed to initialize project sync: ${String(error)}`, 'useProjectSync');
    });
  }, []);
  useProjectSyncCore(undefined);
}
