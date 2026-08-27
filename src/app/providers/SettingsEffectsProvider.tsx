import { useEffect, useLayoutEffect } from "react";
import { i18n } from "@/app/i18n";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import {
  applySmoothScrollSetting,
  syncColorThemePreset,
} from "@/features/application/settings/appearanceRuntime";
import { getThemeModeForPreset } from "@/features/application/settings/colorThemePresets";
import {
    resolveThemeTokens,
    type ResolvedThemeTokens,
} from "@/shared/theme/themeTokens";

import { useWindowDecorationEffect } from "@/features/application/window/useWindowDecorations";
import { SettingsSyncCoordinator } from "@/features/application/settings/settingsSyncCoordinator";

export function applyThemeTokens(root: HTMLElement, tokens: ResolvedThemeTokens): void {
    const set = (name: string, value: string) => root.style.setProperty(name, value);
    const isDark = tokens.mode !== "light";

    root.classList.toggle("dark", isDark);
    root.style.colorScheme = isDark ? "dark" : "light";

    set("--background", tokens.workbenchBg);
    set("--foreground", tokens.foreground);
    set("--card", tokens.surfaceRaised);
    set("--card-foreground", tokens.foreground);
    set("--popover", tokens.surfaceRaised);
    set("--popover-foreground", tokens.foreground);
    set("--primary", tokens.accent);
    set("--primary-foreground", tokens.primaryForeground);
    set("--secondary", tokens.surfaceRaised);
    set("--secondary-foreground", tokens.foreground);
    set("--muted", tokens.surfaceSunken);
    set("--muted-foreground", tokens.mutedForeground);
    set("--accent", tokens.accentSoft);
    set("--accent-foreground", tokens.foreground);
    set("--destructive", tokens.status.danger);
    set("--border", tokens.border);
    set("--input", tokens.inputBorder);
    set("--ring", tokens.ring);
    set("--chart-1", tokens.accent);
    set("--chart-2", tokens.status.success);
    set("--chart-3", tokens.status.info);
    set("--chart-4", tokens.status.warning);
    set("--chart-5", tokens.status.danger);

    set("--sidebar", tokens.sidebarBg);
    set("--sidebar-foreground", tokens.foreground);
    set("--sidebar-primary", tokens.accent);
    set("--sidebar-primary-foreground", tokens.primaryForeground);
    set("--sidebar-accent", tokens.accentSoft);
    set("--sidebar-accent-foreground", tokens.foreground);
    set("--sidebar-border", tokens.border);
    set("--sidebar-ring", tokens.ring);

    set("--workbench-bg", tokens.workbenchBg);
    set("--sidebar-bg", tokens.sidebarBg);
    set("--panel-bg", tokens.panelBg);
    set("--surface-raised", tokens.surfaceRaised);
    set("--surface-sunken", tokens.surfaceSunken);
    set("--panel-header-bg", tokens.panelHeaderBg);
    set("--strong-border", tokens.border);
    set("--accent-color", tokens.accent);
    set("--grid-lines", tokens.grid);
    set("--node-base", tokens.nodeBg);
    set("--node-border", tokens.border);
    set("--node-header-bg", tokens.surfaceRaised);
    set("--node-header-fg", tokens.nodeForeground);
    set(
        "--node-shadow",
        isDark
            ? "0 10px 28px rgb(2 6 23 / 0.32), 0 1px 2px rgb(2 6 23 / 0.42)"
            : "0 9px 24px rgb(15 23 42 / 0.10), 0 1px 2px rgb(15 23 42 / 0.08)",
    );
    set("--connection-lines", tokens.connection);
    set("--selection-region", tokens.selection);
    set("--titlebar-bg", tokens.sidebarBg);
    set("--border-color", tokens.border);
    set("--text-primary", tokens.foreground);
    set("--text-secondary", tokens.mutedForeground);
    set("--hover-bg", tokens.accentSoft);
    set("--accent-color-hover", tokens.accentHover);
    set("--accent-color-soft", tokens.accentSoft);

    set("--interactive-hover", `color-mix(in srgb, ${tokens.accent} 9%, transparent)`);
    set("--interactive-hover-strong", `color-mix(in srgb, ${tokens.accent} 14%, transparent)`);
    set("--interactive-active", `color-mix(in srgb, ${tokens.accent} 12%, transparent)`);
    set("--interactive-divider", `color-mix(in srgb, ${tokens.border} 68%, transparent)`);
    set("--sidebar-section-bg", `color-mix(in srgb, ${tokens.foreground} 5%, transparent)`);
    set("--sidebar-section-active", tokens.accentSoft);
    set("--sidebar-hover", `color-mix(in srgb, ${tokens.accent} 9%, transparent)`);
    set("--sidebar-item-active", `color-mix(in srgb, ${tokens.accent} 12%, transparent)`);
    set("--sidebar-divider", `color-mix(in srgb, ${tokens.border} 68%, transparent)`);

    set("--status-success", tokens.status.success);
    set("--status-warning", tokens.status.warning);
    set("--status-danger", tokens.status.danger);
    set("--status-info", tokens.status.info);
    set("--pin-exec", tokens.pins.exec);
    set("--pin-numeric", tokens.pins.numeric);
    set("--pin-boolean", tokens.pins.boolean);
    set("--pin-text", tokens.pins.text);
    set("--pin-temporal", tokens.pins.temporal);
    set("--pin-table", tokens.pins.table);
    set("--pin-object", tokens.pins.object);
}

export const SettingsEffectsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const theme = useSettingsStore((s) => s.theme);
    const language = useSettingsStore((s) => s.appearance.language);
    const colorTheme = useSettingsStore((s) => s.appearance.colorTheme);
    const smoothScroll = useSettingsStore((s) => s.appearance.smoothScroll);
    const isLoading = useSettingsStore((s) => s.isLoading);
    const updateTheme = useSettingsStore((s) => s.updateTheme);
    const updateAppearance = useSettingsStore((s) => s.updateAppearance);
    const load = useSettingsStore((s) => s.load);

    useWindowDecorationEffect();

    useEffect(() => {
        load();
    }, [load]);

    useEffect(() => {
        const coordinator = new SettingsSyncCoordinator();
        void coordinator.start();
        return () => coordinator.stop();
    }, []);

    useEffect(() => {
        if (i18n.language !== language) {
            void i18n.changeLanguage(language);
        }
    }, [language]);

    useEffect(() => {
        if (isLoading) return;
        const mode = getThemeModeForPreset(colorTheme);
        const state = useSettingsStore.getState();
        const remembered = mode === "light" ? state.appearance.lastLightColorTheme : state.appearance.lastDarkColorTheme;
        if (remembered !== colorTheme) {
            updateAppearance(mode === "light"
                ? { lastLightColorTheme: colorTheme }
                : { lastDarkColorTheme: colorTheme });
        }
        syncColorThemePreset(colorTheme, updateTheme);
    }, [colorTheme, updateAppearance, updateTheme, isLoading]);

    useEffect(() => {
        applySmoothScrollSetting(smoothScroll);
    }, [smoothScroll]);

    // 使用 useLayoutEffect 确保 CSS 变量在 DOM 更新前同步应用。
    useLayoutEffect(() => {
        applyThemeTokens(document.documentElement, resolveThemeTokens(theme));
    }, [theme]);

    return <>{children}</>;
};
