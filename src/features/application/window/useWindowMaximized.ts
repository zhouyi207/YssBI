import { useEffect, useState } from "react";
import { currentAppWindow } from "@/services/platform/appWindow";
import { logger } from "@/features/application/observability/appLogger";

/** 跟踪当前 Tauri 窗口是否最大化（用于最大化按钮 tooltip） */
export function useWindowMaximized(logTag = "Window") {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | null = null;

    const setup = async () => {
      const win = currentAppWindow();
      const maximized = await win.isMaximized();
      if (!disposed && maximized.ok) setIsMaximized(maximized.value);

      const unlisten = await win.onResized(() => {
        if (disposed) return;
        void win.isMaximized().then((result) => {
          if (result.ok && !disposed) setIsMaximized(result.value);
          else if (!result.ok) logger.sys.warn("window maximized state unavailable", logTag);
        });
      });

      if (disposed) {
        if (unlisten.ok) unlisten.value();
      } else if (unlisten.ok) {
        cleanup = unlisten.value;
      }
    };

    void setup();

    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [logTag]);

  return isMaximized;
}
