import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { logger } from "@/utils/appLogger";
import { resolveWindowDecorations, usesCustomTitleBar } from "./windowDecorationPolicy";

/** Sync OS window decorations with appearance.titleBarStyle for the current webview. */
export function useWindowDecorationEffect(): void {
    const titleBarStyle = useSettingsStore((s) => s.appearance.titleBarStyle);

    useEffect(() => {
        const native = resolveWindowDecorations(titleBarStyle);
        void getCurrentWindow()
            .setDecorations(native)
            .catch((error) => {
                logger.app.warn(
                    `Failed to set window decorations: ${error instanceof Error ? error.message : String(error)}`,
                    "Window",
                );
            });
    }, [titleBarStyle]);
}

/** Whether the current window should render custom title bar chrome. */
export function useCustomTitleBar(): boolean {
    const titleBarStyle = useSettingsStore((s) => s.appearance.titleBarStyle);
    return usesCustomTitleBar(titleBarStyle);
}
