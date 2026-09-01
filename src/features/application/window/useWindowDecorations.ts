import { useEffect } from "react";
import { currentAppWindow } from "@/services/platform/appWindow";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { logger } from "@/features/application/observability/appLogger";
import { resolveWindowDecorations, usesCustomTitleBar } from "./windowDecorationPolicy";

/** Sync OS window decorations with appearance.titleBarStyle for the current webview. */
export function useWindowDecorationEffect(): void {
  const titleBarStyle = useSettingsStore((s) => s.appearance.titleBarStyle);

  useEffect(() => {
    const native = resolveWindowDecorations(titleBarStyle);
    void currentAppWindow()
      .setDecorations(native)
      .then((result) => {
        if (!result.ok) logger.app.warn("window decorations unavailable", "Window");
      });
  }, [titleBarStyle]);
}

/** Whether the current window should render custom title bar chrome. */
export function useCustomTitleBar(): boolean {
  const titleBarStyle = useSettingsStore((s) => s.appearance.titleBarStyle);
  return usesCustomTitleBar(titleBarStyle);
}
