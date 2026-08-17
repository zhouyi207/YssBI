// src/features/application/initialization/useProjectSync.ts
// Application 层：协调 useEditor 与 Core 的 ProjectListener

import { useEffect, useRef } from 'react';
import { ProjectListener } from '@/features/core/sync/listeners/ProjectListener';
import { SingletonManager } from '@/features/core/sync/utils/singletonManager';
import { logger } from '@/utils/appLogger';

const LISTENER_KEY = 'project-listener';

/**
 * 项目同步核心逻辑
 */
function useProjectSyncCore() {
  const isSetupRef = useRef(false);

  useEffect(() => {
    if (isSetupRef.current) return;
    isSetupRef.current = true;
    let cancelled = false;
    let acquired = false;

    const release = () => {
      SingletonManager.decrementRef(LISTENER_KEY, (instance: ProjectListener) => {
        instance.stop();
      });
    };

    const setup = async () => {
      await SingletonManager.getInstance(
        LISTENER_KEY,
        async () => {
          logger.sys.debug('Creating new listener instance', 'useProjectSync');
          const newListener = new ProjectListener();
          await newListener.start();
          return newListener;
        },
      );

      if (cancelled) {
        release();
        return;
      }

      acquired = true;
    };

    void setup().catch((error) => {
      logger.sys.error(
        `Failed to start project listener: ${String(error)}`,
        'useProjectSync',
      );
    });

    return () => {
      cancelled = true;
      isSetupRef.current = false;
      if (acquired) {
        release();
      }
    };
  }, []);
}

/**
 * 启动项目事件监听（EditorWindow / DatabaseEditorWindow 等共用）。
 * 项目投影 hydration 由各入口显式编排。
 */
export function useProjectSync() {
  useProjectSyncCore();
}
