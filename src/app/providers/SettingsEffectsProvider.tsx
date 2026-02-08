// src/app/providers/SettingsEffectsProvider.tsx
import { useEffect } from "react";
import { useSettingsStore } from "@/stores/settingsStore";

export const SettingsEffectsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const theme = useSettingsStore((s) => s.theme);
    const load = useSettingsStore((s) => s.load);

    useEffect(() => {
        load();
    }, [load]);

    useEffect(() => {
        const root = document.documentElement;

        // 主要背景色
        root.style.setProperty("--workbench-bg", theme.workbenchBackground);
        root.style.setProperty("--sidebar-bg", theme.sidebarBackground);
        root.style.setProperty("--accent-color", theme.accentColor);
        root.style.setProperty("--grid-lines", theme.gridLines);
        root.style.setProperty("--node-base", theme.nodeBase);
        root.style.setProperty("--connection-lines", theme.connectionLines);
        root.style.setProperty("--selection-region", theme.selectionRegion);

        // Pin 类型颜色
        root.style.setProperty("--exec-color", theme.execColor);
        root.style.setProperty("--int-color", theme.intColor);
        root.style.setProperty("--float-color", theme.floatColor);
        root.style.setProperty("--bool-color", theme.boolColor);
        root.style.setProperty("--string-color", theme.stringColor);
        root.style.setProperty("--date-color", theme.dateColor);
        root.style.setProperty("--datetime-color", theme.datetimeColor);
        root.style.setProperty("--dataframe-color", theme.dataframeColor);
        root.style.setProperty("--object-color", theme.objectColor);
        root.style.setProperty("--array-color", theme.arrayColor);

        // 添加Plot窗口需要的CSS变量
        root.style.setProperty("--titlebar-bg", theme.sidebarBackground);
        root.style.setProperty("--border-color", theme.gridLines);
        root.style.setProperty("--text-primary", theme.execColor);
        root.style.setProperty("--text-secondary", theme.connectionLines);
        root.style.setProperty("--hover-bg", "rgba(255, 255, 255, 0.05)");

        // 计算派生颜色
        root.style.setProperty("--accent-color-hover", theme.accentColor + "cc");

        console.log("[ThemeProvider] CSS variables applied");
    }, [theme]);

    return <>{children}</>;
};
