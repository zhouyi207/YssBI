import React, { createContext, useContext, useState, useEffect, useCallback, ReactNode } from "react";
import {
    SettingsService,
} from "@/services/settings";
import {
    AppSettings,
    ThemeSettings,
    EditorSettings,
    AppearanceSettings,
    ProjectSettings,
    WindowSettings,
} from "@/shared/types/settings";
import {
    DEFAULT_APPEARANCE,
    DEFAULT_EDITOR,
    DEFAULT_THEME,
    DEFAULT_PROJECT,
    DEFAULT_WINDOW,
} from "@/app/appConfig/default";

interface SettingsContextValue {
    // 状态
    theme: ThemeSettings;
    editor: EditorSettings;
    appearance: AppearanceSettings;
    project: ProjectSettings;
    isLoading: boolean;

    // 更新方法
    updateTheme: (updates: Partial<ThemeSettings>) => void;
    updateEditor: (updates: Partial<EditorSettings>) => void;
    updateAppearance: (updates: Partial<AppearanceSettings>) => void;
    updateProject: (updates: Partial<ProjectSettings>) => void;

    // 恢复默认方法
    resetAllToDefaults: () => Promise<void>;
    resetThemeToDefaults: () => Promise<void>;
    resetEditorToDefaults: () => Promise<void>;
    resetAppearanceToDefaults: () => Promise<void>;

    // 重新加载设置
    reloadSettings: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export const ThemeProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [theme, setTheme] = useState<ThemeSettings>(DEFAULT_THEME);
    const [editor, setEditor] = useState<EditorSettings>(DEFAULT_EDITOR);
    const [appearance, setAppearance] = useState<AppearanceSettings>(DEFAULT_APPEARANCE);
    const [project, setProject] = useState<ProjectSettings>(DEFAULT_PROJECT);
    const [isLoading, setIsLoading] = useState(true);

    // 加载设置
    const loadSettings = useCallback(async () => {
        try {
            setIsLoading(true);
            const settings = await SettingsService.loadSettings();
            setTheme(settings.theme);
            setEditor(settings.editor);
            setAppearance(settings.appearance);
            setProject(settings.project);
            console.log("[ThemeProvider] Settings loaded into context");
        } catch (error) {
            console.error("[ThemeProvider] Failed to load settings:", error);
        } finally {
            setIsLoading(false);
        }
    }, []);

    // 初始化时加载设置
    useEffect(() => {
        loadSettings();
    }, [loadSettings]);

    // 保存设置的防抖
    const saveSettingsDebounced = useCallback(
        (() => {
            let timeoutId: ReturnType<typeof setTimeout> | null = null;
            return (settings: AppSettings) => {
                if (timeoutId) clearTimeout(timeoutId);
                timeoutId = setTimeout(() => {
                    SettingsService.saveSettings(settings).catch(console.error);
                }, 500); // 500ms 防抖
            };
        })(),
        []
    );

    // 更新主题
    const updateTheme = useCallback((updates: Partial<ThemeSettings>) => {
        setTheme(prev => {
            const newTheme = { ...prev, ...updates };
            // 异步保存
            const cached = SettingsService.getCachedSettings();
            if (cached) {
                saveSettingsDebounced({ ...cached, theme: newTheme });
            }
            return newTheme;
        });
    }, [saveSettingsDebounced]);

    // 更新编辑器设置
    const updateEditor = useCallback((updates: Partial<EditorSettings>) => {
        setEditor(prev => {
            const newEditor = { ...prev, ...updates };
            const cached = SettingsService.getCachedSettings();
            if (cached) {
                saveSettingsDebounced({ ...cached, editor: newEditor });
            }
            return newEditor;
        });
    }, [saveSettingsDebounced]);

    // 更新外观设置
    const updateAppearance = useCallback((updates: Partial<AppearanceSettings>) => {
        setAppearance(prev => {
            const newAppearance = { ...prev, ...updates };
            const cached = SettingsService.getCachedSettings();
            if (cached) {
                saveSettingsDebounced({ ...cached, appearance: newAppearance });
            }
            return newAppearance;
        });
    }, [saveSettingsDebounced]);

    // 更新项目设置
    const updateProject = useCallback((updates: Partial<ProjectSettings>) => {
        setProject(prev => {
            const newProject = { ...prev, ...updates };
            const cached = SettingsService.getCachedSettings();
            if (cached) {
                saveSettingsDebounced({ ...cached, project: newProject });
            }
            return newProject;
        });
    }, [saveSettingsDebounced]);

    // 恢复所有默认设置
    const resetAllToDefaults = useCallback(async () => {
        const defaults = await SettingsService.resetToDefaults();
        setTheme(defaults.theme);
        setEditor(defaults.editor);
        setAppearance(defaults.appearance);
        setProject(defaults.project);
    }, []);

    // 恢复默认主题设置
    const resetThemeToDefaults = useCallback(async () => {
        const settings = await SettingsService.resetThemeToDefaults();
        setTheme(settings.theme);
    }, []);

    // 恢复默认编辑器设置
    const resetEditorToDefaults = useCallback(async () => {
        const settings = await SettingsService.resetEditorToDefaults();
        setEditor(settings.editor);
    }, []);

    // 恢复默认外观设置
    const resetAppearanceToDefaults = useCallback(async () => {
        const settings = await SettingsService.resetAppearanceToDefaults();
        setAppearance(settings.appearance);
    }, []);

    // 重新加载设置
    const reloadSettings = useCallback(async () => {
        await loadSettings();
    }, [loadSettings]);

    // 应用主题到 CSS 变量
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

    const contextValue: SettingsContextValue = {
        theme,
        editor,
        appearance,
        project,
        isLoading,
        updateTheme,
        updateEditor,
        updateAppearance,
        updateProject,
        resetAllToDefaults,
        resetThemeToDefaults,
        resetEditorToDefaults,
        resetAppearanceToDefaults,
        reloadSettings,
    };

    return (
        <SettingsContext.Provider value={contextValue}>
            {children}
        </SettingsContext.Provider>
    );
};

// 为了向后兼容，保留 useTheme hook
export const useTheme = () => {
    const ctx = useContext(SettingsContext);
    if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
    return {
        theme: ctx.theme,
        updateTheme: ctx.updateTheme,
    };
};

// 新的完整设置 hook
export const useSettings = () => {
    const ctx = useContext(SettingsContext);
    if (!ctx) throw new Error("useSettings must be used within ThemeProvider");
    return ctx;
};
