import { invoke } from "@tauri-apps/api/core";

// ==================== 类型定义 ====================

export interface ThemeSettings {
    workbenchBackground: string;
    sidebarBackground: string;
    accentColor: string;
    gridLines: string;
    nodeBase: string;
    connectionLines: string;
    selectionRegion: string;
    // Pin & Type Colors
    execColor: string;
    intColor: string;
    floatColor: string;
    boolColor: string;
    stringColor: string;
    dateColor: string;
    datetimeColor: string;
    dataframeColor: string;
    objectColor: string;
    arrayColor: string;
}

export interface EditorSettings {
    showGrid: boolean;
    autoSave: boolean;
    snapToGrid: boolean;
    fontSize: number;
}

export interface AppearanceSettings {
    colorTheme: string;
    activityBarPosition: string;
    smoothScroll: boolean;
}

export interface ProjectSettings {
    projectName: string;
    exportPath: string;
}

export interface WindowSettings {
    width: number;
    height: number;
    x: number | null;
    y: number | null;
    isMaximized: boolean;
}

export interface AppSettings {
    theme: ThemeSettings;
    editor: EditorSettings;
    appearance: AppearanceSettings;
    project: ProjectSettings;
    window: WindowSettings;
}

// 深度部分类型，允许嵌套属性也是可选的
export interface PartialAppSettings {
    theme?: Partial<ThemeSettings>;
    editor?: Partial<EditorSettings>;
    appearance?: Partial<AppearanceSettings>;
    project?: Partial<ProjectSettings>;
    window?: Partial<WindowSettings>;
}

// ==================== 默认值定义 ====================

export const DEFAULT_THEME: ThemeSettings = {
    workbenchBackground: "#121212",
    sidebarBackground: "#181818",
    accentColor: "#0078d4",
    gridLines: "#252525",
    nodeBase: "#2d2d2d",
    connectionLines: "#6b6b6b",
    selectionRegion: "#0078d433",
    execColor: "#ffffff",
    intColor: "#35b2b2",
    floatColor: "#9ecd4d",
    boolColor: "#e06c75",
    stringColor: "#e5c07b",
    dateColor: "#c678dd",
    datetimeColor: "#c678dd",
    dataframeColor: "#61afef",
    objectColor: "#abb2bf",
    arrayColor: "#d19a66",
};

export const DEFAULT_EDITOR: EditorSettings = {
    showGrid: true,
    autoSave: true,
    snapToGrid: true,
    fontSize: 12,
};

export const DEFAULT_APPEARANCE: AppearanceSettings = {
    colorTheme: "Dark Modern (Default)",
    activityBarPosition: "Left",
    smoothScroll: true,
};

export const DEFAULT_PROJECT: ProjectSettings = {
    projectName: "YssBI Project",
    exportPath: "",
};

export const DEFAULT_WINDOW: WindowSettings = {
    width: 1600,
    height: 900,
    x: null,
    y: null,
    isMaximized: false,
};

export const DEFAULT_SETTINGS: AppSettings = {
    theme: DEFAULT_THEME,
    editor: DEFAULT_EDITOR,
    appearance: DEFAULT_APPEARANCE,
    project: DEFAULT_PROJECT,
    window: DEFAULT_WINDOW,
};

// ==================== 设置服务 ====================

export class SettingsService {
    private static settingsCache: AppSettings | null = null;

    /**
     * 加载设置，如果文件不存在则返回默认设置
     */
    static async loadSettings(): Promise<AppSettings> {
        try {
            const settings = await invoke<AppSettings>("load_settings");
            this.settingsCache = settings;
            console.info("Settings loaded successfully via backend");
            return settings;
        } catch (error) {
            console.error("Error loading settings via backend:", error);
            this.settingsCache = { ...DEFAULT_SETTINGS };
            return this.settingsCache;
        }
    }

    /**
     * 保存设置到后端
     */
    static async saveSettings(settings: AppSettings): Promise<void> {
        try {
            await invoke("save_settings", { settings });
            this.settingsCache = settings;
            console.log("Settings saved successfully via backend");
        } catch (error) {
            console.error("Error saving settings via backend:", error);
            throw error;
        }
    }

    /**
     * 更新部分设置
     */
    static async updateSettings(updates: PartialAppSettings): Promise<AppSettings> {
        const currentSettings = this.settingsCache || await this.loadSettings();

        const newSettings: AppSettings = {
            theme: updates.theme ? { ...currentSettings.theme, ...updates.theme } : currentSettings.theme,
            editor: updates.editor ? { ...currentSettings.editor, ...updates.editor } : currentSettings.editor,
            appearance: updates.appearance ? { ...currentSettings.appearance, ...updates.appearance } : currentSettings.appearance,
            project: updates.project ? { ...currentSettings.project, ...updates.project } : currentSettings.project,
            window: updates.window ? { ...currentSettings.window, ...updates.window } : currentSettings.window,
        };

        await this.saveSettings(newSettings);
        return newSettings;
    }

    /**
     * 更新主题设置
     */
    static async updateTheme(themeUpdates: Partial<ThemeSettings>): Promise<AppSettings> {
        return this.updateSettings({ theme: themeUpdates });
    }

    /**
     * 更新编辑器设置
     */
    static async updateEditor(editorUpdates: Partial<EditorSettings>): Promise<AppSettings> {
        return this.updateSettings({ editor: editorUpdates });
    }

    /**
     * 更新外观设置
     */
    static async updateAppearance(appearanceUpdates: Partial<AppearanceSettings>): Promise<AppSettings> {
        return this.updateSettings({ appearance: appearanceUpdates });
    }

    /**
     * 更新项目设置
     */
    static async updateProject(projectUpdates: Partial<ProjectSettings>): Promise<AppSettings> {
        return this.updateSettings({ project: projectUpdates });
    }

    /**
     * 更新窗口设置
     */
    static async updateWindow(windowUpdates: Partial<WindowSettings>): Promise<AppSettings> {
        return this.updateSettings({ window: windowUpdates });
    }

    /**
     * 恢复默认设置
     */
    static async resetToDefaults(): Promise<AppSettings> {
        await this.saveSettings(DEFAULT_SETTINGS);
        return { ...DEFAULT_SETTINGS };
    }

    /**
     * 恢复默认主题设置
     */
    static async resetThemeToDefaults(): Promise<AppSettings> {
        const currentSettings = this.settingsCache || await this.loadSettings();
        const newSettings: AppSettings = {
            ...currentSettings,
            theme: { ...DEFAULT_THEME },
        };
        await this.saveSettings(newSettings);
        return newSettings;
    }

    /**
     * 恢复默认编辑器设置
     */
    static async resetEditorToDefaults(): Promise<AppSettings> {
        const currentSettings = this.settingsCache || await this.loadSettings();
        const newSettings: AppSettings = {
            ...currentSettings,
            editor: { ...DEFAULT_EDITOR },
        };
        await this.saveSettings(newSettings);
        return newSettings;
    }

    /**
     * 恢复默认外观设置
     */
    static async resetAppearanceToDefaults(): Promise<AppSettings> {
        const currentSettings = this.settingsCache || await this.loadSettings();
        const newSettings: AppSettings = {
            ...currentSettings,
            appearance: { ...DEFAULT_APPEARANCE },
        };
        await this.saveSettings(newSettings);
        return newSettings;
    }

    /**
     * 获取缓存的设置（同步）
     */
    static getCachedSettings(): AppSettings | null {
        return this.settingsCache;
    }

    /**
     * 获取默认设置
     */
    static getDefaultSettings(): AppSettings {
        return { ...DEFAULT_SETTINGS };
    }
}
