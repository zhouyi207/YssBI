import { useEffect, useLayoutEffect } from "react";
import { i18n } from "@/app/i18n";
import { subscribeClientSettingsCrossWindow, useSettingsStore } from "@/features/core/settings/settingsStore";
import {
  applySmoothScrollSetting,
  syncColorThemePreset,
} from "@/features/application/settings/appearanceRuntime";

import { useWindowDecorationEffect } from "@/features/application/window/useWindowDecorations";

export const SettingsEffectsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const theme = useSettingsStore((s) => s.theme);
    const language = useSettingsStore((s) => s.appearance.language);
    const colorTheme = useSettingsStore((s) => s.appearance.colorTheme);
    const smoothScroll = useSettingsStore((s) => s.appearance.smoothScroll);
    const isLoading = useSettingsStore((s) => s.isLoading);
    const updateTheme = useSettingsStore((s) => s.updateTheme);
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
        syncColorThemePreset(colorTheme, updateTheme);
    }, [colorTheme, updateTheme, isLoading]);

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
                workbenchForeground: "#f8fafc",
                panelBackground: theme.sidebarBackground,
                panelForeground: "#f8fafc",
                mutedForeground: "#a1a1aa",
                border: "rgba(255, 255, 255, 0.10)",
                hoverBackground: "rgba(255, 255, 255, 0.05)",
                nodeBase: theme.nodeBase,
                nodeBorder: "rgba(255, 255, 255, 0.12)",
                nodeHeaderBackground: "rgba(255, 255, 255, 0.05)",
                nodeHeaderForeground: "#d4d4d8",
                nodeShadow: "0 18px 40px rgb(0 0 0 / 0.35)",
            }
            : {
                workbenchForeground: "#111827",
                panelBackground: "#ffffff",
                panelForeground: "#111827",
                mutedForeground: "#64748b",
                border: "#e5e7eb",
                hoverBackground: "rgba(15, 23, 42, 0.06)",
                nodeBase: theme.nodeBase === theme.workbenchBackground ? "#f8fafc" : theme.nodeBase,
                nodeBorder: "#cbd5e1",
                nodeHeaderBackground: "#eef2ff",
                nodeHeaderForeground: "#1e293b",
                nodeShadow: "0 16px 32px rgb(15 23 42 / 0.12)",
            };

        root.classList.toggle("dark", isDark);
        root.style.colorScheme = isDark ? "dark" : "light";

        // 主要背景色
        root.style.setProperty("--workbench-bg", theme.workbenchBackground);
        root.style.setProperty("--sidebar-bg", theme.sidebarBackground);
        root.style.setProperty("--workbench-fg", surface.workbenchForeground);
        root.style.setProperty("--panel-bg", surface.panelBackground);
        root.style.setProperty("--panel-fg", surface.panelForeground);
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
        root.style.setProperty("--accent-color-hover", theme.accentColor + "cc");

    }, [theme]);

    return <>{children}</>;
};
