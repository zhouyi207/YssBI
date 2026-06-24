import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { logger } from '@/utils/appLogger';

/** 跟踪当前 Tauri 窗口是否最大化（用于最大化按钮 tooltip） */
export function useWindowMaximized(logTag = 'Window') {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | null = null;

    const setup = async () => {
      const win = getCurrentWindow();
      const maximized = await win.isMaximized().catch(() => false);
      if (!disposed) setIsMaximized(maximized);

      const unlisten = await win.onResized(async () => {
        if (disposed) return;
        try {
          setIsMaximized(await win.isMaximized());
        } catch (e) {
          logger.sys.warn(`Failed to check maximized state: ${String(e)}`, logTag);
        }
      });

      if (disposed) unlisten();
      else cleanup = unlisten;
    };

    setup().catch((e) => logger.app.error(String(e), logTag));

    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [logTag]);

  return isMaximized;
}
