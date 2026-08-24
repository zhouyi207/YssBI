import { useEffect, useLayoutEffect } from "react";
import { i18n } from "@/app/i18n";
import { subscribeClientSettingsCrossWindow, useSettingsStore } from "@/features/core/settings/settingsStore";
import {
  applySmoothScrollSetting,
  syncColorThemePreset,
} from "@/features/application/settings/appearanceRuntime";
import { getThemeModeForPreset } from "@/features/application/settings/colorThemePresets";

import { useWindowDecorationEffect } from "@/features/application/window/useWindowDecorations";

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
        let cancelled = false;
        let unlisten: (() => void) | undefined;
        void subscribeClientSettingsCrossWindow().then((fn) => {
            if (!cancelled) unlisten = fn;
        });
        return () => {
            cancelled = true;
            unlisten?.();
        };
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

    // 使用 useLayoutEffect 确保 CSS 变量在 DOM 更新前同步应用
    // 这样可以避免 TabBar 等组件渲染时读取到旧的 CSS 变量值
    useLayoutEffect(() => {
        const root = document.documentElement;
        const isDark = theme.mode !== "light";
        const surface = isDark
            ? {
                workbenchForeground: "#e7ebf3",
                panelBackground: theme.sidebarBackground,
                mutedForeground: "#929db0",
                border: "rgba(148, 163, 184, 0.16)",
                hoverBackground: "color-mix(in srgb, var(--accent-color) 11%, transparent)",
                raisedBackground: `color-mix(in srgb, ${theme.sidebarBackground} 98%, white)`,
                sunkenBackground: `color-mix(in srgb, ${theme.workbenchBackground} 92%, black)`,
                panelHeaderBackground: theme.sidebarBackground,
                nodeBase: theme.nodeBase,
                nodeBorder: "rgba(148, 163, 184, 0.22)",
                nodeHeaderBackground: "rgba(255, 255, 255, 0.035)",
                nodeHeaderForeground: "#e7ebf3",
                nodeShadow: "0 10px 28px rgb(2 6 23 / 0.32), 0 1px 2px rgb(2 6 23 / 0.42)",
            }
            : {
                workbenchForeground: "#202938",
                panelBackground: theme.sidebarBackground,
                mutedForeground: "#596579",
                border: "#d7dee9",
                hoverBackground: "color-mix(in srgb, var(--accent-color) 9%, transparent)",
                raisedBackground: `color-mix(in srgb, ${theme.workbenchBackground} 35%, white)`,
                sunkenBackground: `color-mix(in srgb, ${theme.sidebarBackground} 82%, ${theme.workbenchBackground})`,
                panelHeaderBackground: theme.sidebarBackground,
                nodeBase: theme.nodeBase === theme.workbenchBackground ? "#ffffff" : theme.nodeBase,
                nodeBorder: "#cbd4e1",
                nodeHeaderBackground: "#f4f6f9",
                nodeHeaderForeground: "#293241",
                nodeShadow: "0 9px 24px rgb(15 23 42 / 0.10), 0 1px 2px rgb(15 23 42 / 0.08)",
            };

        root.classList.toggle("dark", isDark);
        root.style.colorScheme = isDark ? "dark" : "light";

        // 主要背景色
        root.style.setProperty("--workbench-bg", theme.workbenchBackground);
        root.style.setProperty("--sidebar-bg", theme.sidebarBackground);
        root.style.setProperty("--panel-bg", surface.panelBackground);
        root.style.setProperty("--surface-raised", surface.raisedBackground);
        root.style.setProperty("--surface-sunken", surface.sunkenBackground);
        root.style.setProperty("--panel-header-bg", surface.panelHeaderBackground);
        root.style.setProperty("--strong-border", surface.border);
        root.style.setProperty("--accent-color", theme.accentColor);
        root.style.setProperty("--grid-lines", theme.gridLines);
        root.style.setProperty("--node-base", surface.nodeBase);
        root.style.setProperty("--node-border", surface.nodeBorder);
        root.style.setProperty("--node-header-bg", surface.nodeHeaderBackground);
        root.style.setProperty("--node-header-fg", surface.nodeHeaderForeground);
        root.style.setProperty("--node-shadow", surface.nodeShadow);
        root.style.setProperty("--connection-lines", theme.connectionLines);
        root.style.setProperty("--selection-region", theme.selectionRegion);

        // Pin 类型颜色（保留精度）
        root.style.setProperty("--exec-color", theme.execColor);
        root.style.setProperty("--int32-color", theme.int32Color);
        root.style.setProperty("--int64-color", theme.int64Color);
        root.style.setProperty("--float32-color", theme.float32Color);
        root.style.setProperty("--float64-color", theme.float64Color);
        root.style.setProperty("--bool-color", theme.boolColor);
        root.style.setProperty("--string-color", theme.stringColor);
        root.style.setProperty("--date-color", theme.dateColor);
        root.style.setProperty("--datetime-color", theme.datetimeColor);
        root.style.setProperty("--categorical-color", theme.categoricalColor);
        root.style.setProperty("--dataframe-color", theme.dataframeColor);
        root.style.setProperty("--dataseries-color", theme.dataseriesColor);
        root.style.setProperty("--object-color", theme.objectColor);
        root.style.setProperty("--any-color", theme.anyColor);
        root.style.setProperty("--array-color", theme.arrayColor);
        root.style.setProperty("--struct-color", theme.structColor);

        // 添加Plot窗口需要的CSS变量
        root.style.setProperty("--titlebar-bg", theme.sidebarBackground);
        root.style.setProperty("--border-color", theme.gridLines);
        root.style.setProperty("--text-primary", surface.workbenchForeground);
        root.style.setProperty("--text-secondary", surface.mutedForeground);
        root.style.setProperty("--hover-bg", surface.hoverBackground);

        // 计算派生颜色
        root.style.setProperty(
            "--accent-color-hover",
            `color-mix(in srgb, ${theme.accentColor} 84%, ${isDark ? "white" : "black"})`,
        );
        root.style.setProperty("--accent-color-soft", `color-mix(in srgb, ${theme.accentColor} 12%, transparent)`);

    }, [theme]);

    return <>{children}</>;
};
