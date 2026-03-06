import { useEffect, useLayoutEffect } from "react";
import { useSettingsStore } from "@/features/core/settings/settingsStore";

export const SettingsEffectsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const theme = useSettingsStore((s) => s.theme);
    const load = useSettingsStore((s) => s.load);

    useEffect(() => {
        load();
    }, [load]);

    // 使用 useLayoutEffect 确保 CSS 变量在 DOM 更新前同步应用
    // 这样可以避免 TabBar 等组件渲染时读取到旧的 CSS 变量值
    useLayoutEffect(() => {
        const root = document.documentElement;

        // 主要背景色
        root.style.setProperty("--workbench-bg", theme.workbenchBackground);
        root.style.setProperty("--sidebar-bg", theme.sidebarBackground);
        root.style.setProperty("--accent-color", theme.accentColor);
        root.style.setProperty("--grid-lines", theme.gridLines);
        root.style.setProperty("--node-base", theme.nodeBase);
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
        root.style.setProperty("--text-primary", theme.execColor);
        root.style.setProperty("--text-secondary", theme.connectionLines);
        root.style.setProperty("--hover-bg", "rgba(255, 255, 255, 0.05)");

        // 计算派生颜色
        root.style.setProperty("--accent-color-hover", theme.accentColor + "cc");

    }, [theme]);

    return <>{children}</>;
};
