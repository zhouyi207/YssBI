import type { TitleBarStyle } from "@/shared/types/settings";
import { useSettingsStore } from "@/features/core/settings/settingsStore";

/** Tauri `decorations` flag — true when OS native frame is shown. */
export function resolveWindowDecorations(style: TitleBarStyle): boolean {
    return style === "native";
}

export function usesCustomTitleBar(style: TitleBarStyle): boolean {
    return style !== "native";
}

export function readTitleBarStyleFromSettings(): TitleBarStyle {
    return useSettingsStore.getState().appearance.titleBarStyle;
}

export function readWindowDecorationsFromSettings(): boolean {
    return resolveWindowDecorations(readTitleBarStyleFromSettings());
}
