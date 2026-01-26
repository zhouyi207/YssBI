import { writeTextFile, readTextFile, exists, mkdir, BaseDirectory } from "@tauri-apps/plugin-fs";

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
    objectColor: string;
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
    workbenchBackground: "#181818",
    sidebarBackground: "#1f1f1f",
    accentColor: "#007acc",
    gridLines: "#222222",
    nodeBase: "#2b2b2b",
    connectionLines: "#888888",
    selectionRegion: "#007acc33",
    execColor: "#ffffff",
    intColor: "#3592c4",
    floatColor: "#4ab3e3",
    boolColor: "#c94f4f",
    stringColor: "#7bb0a6",
    objectColor: "#9179c9",
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
    width: 1560,
    height: 840,
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

const SETTINGS_FILE = "settings.json";
const SETTINGS_DIR = ""; // 在 AppConfig 目录根下

export class SettingsService {
    private static settingsCache: AppSettings | null = null;

    /**
     * 确保配置目录存在
     */
    private static async ensureConfigDir(): Promise<void> {
        try {
            const dirExists = await exists("", { baseDir: BaseDirectory.AppConfig });
            if (!dirExists) {
                await mkdir("", { baseDir: BaseDirectory.AppConfig, recursive: true });
            }
        } catch (error) {
            console.warn("Could not create config directory:", error);
        }
    }

    /**
     * 获取设置文件路径（用于调试）
     */
    static getSettingsPath(): string {
        return `${BaseDirectory.AppConfig}/${SETTINGS_FILE}`;
    }

    /**
     * 加载设置，如果文件不存在则返回默认设置
     */
    static async loadSettings(): Promise<AppSettings> {
        try {
            await this.ensureConfigDir();
            
            const fileExists = await exists(SETTINGS_FILE, { baseDir: BaseDirectory.AppConfig });
            if (!fileExists) {
                console.log("Settings file not found, using defaults");
                // 第一次运行时保存默认设置
                await this.saveSettings(DEFAULT_SETTINGS);
                this.settingsCache = { ...DEFAULT_SETTINGS };
                return this.settingsCache;
            }

            const content = await readTextFile(SETTINGS_FILE, { baseDir: BaseDirectory.AppConfig });
            const loadedSettings = JSON.parse(content) as Partial<AppSettings>;
            
            // 合并加载的设置与默认设置，确保所有字段都存在
            const mergedSettings: AppSettings = {
                theme: { ...DEFAULT_THEME, ...loadedSettings.theme },
                editor: { ...DEFAULT_EDITOR, ...loadedSettings.editor },
                appearance: { ...DEFAULT_APPEARANCE, ...loadedSettings.appearance },
                project: { ...DEFAULT_PROJECT, ...loadedSettings.project },
                window: { ...DEFAULT_WINDOW, ...loadedSettings.window },
            };

            this.settingsCache = mergedSettings;
            console.log("Settings loaded successfully");
            return mergedSettings;
        } catch (error) {
            console.error("Error loading settings:", error);
            this.settingsCache = { ...DEFAULT_SETTINGS };
            return this.settingsCache;
        }
    }

    /**
     * 保存设置到文件
     */
    static async saveSettings(settings: AppSettings): Promise<void> {
        try {
            await this.ensureConfigDir();
            const content = JSON.stringify(settings, null, 2);
            await writeTextFile(SETTINGS_FILE, content, { baseDir: BaseDirectory.AppConfig });
            this.settingsCache = settings;
            console.log("Settings saved successfully");
        } catch (error) {
            console.error("Error saving settings:", error);
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
