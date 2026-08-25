import { create } from "zustand";
import { emit, listen } from "@tauri-apps/api/event";
import {
    ThemeSettings,
    EditorSettings,
    AppearanceSettings,
    ProjectSettings,
    AppSettings,
} from "@/shared/types/settings";
import {
    DEFAULT_THEME,
    DEFAULT_EDITOR,
    DEFAULT_APPEARANCE,
    DEFAULT_PROJECT,
} from "@/app/appConfig/default";
import { logger } from '@/utils/appLogger';

const SETTINGS_STORAGE_KEY = "yssbi-client-settings-v2";
const LEGACY_SETTINGS_STORAGE_KEY = "yssbi-client-settings";

/** 多 WebView 窗口间同步客户端设置（主题等），与 localStorage 同源 */
export const CLIENT_SETTINGS_UPDATED_EVENT = "client-settings-updated";

let suppressClientSettingsCrossWindowBroadcast = false;

function clientSettingsFingerprint(s: AppSettings): string {
    return JSON.stringify({
        theme: s.theme,
        editor: s.editor,
        appearance: s.appearance,
        project: s.project,
    });
}

function mergeThemeSettings(theme: Partial<ThemeSettings> | undefined): ThemeSettings {
    const source = theme ?? {};
    const defaults = DEFAULT_THEME;
    return {
        mode: source.mode === "light" ? "light" : "dark",
        workbenchBackground: source.workbenchBackground ?? defaults.workbenchBackground,
        sidebarBackground: source.sidebarBackground ?? defaults.sidebarBackground,
        nodeBackground: source.nodeBackground ?? defaults.nodeBackground,
        foreground: source.foreground ?? defaults.foreground,
        mutedForeground: source.mutedForeground ?? defaults.mutedForeground,
        accentColor: source.accentColor ?? defaults.accentColor,
        borderColor: source.borderColor ?? defaults.borderColor,
        gridColor: source.gridColor ?? defaults.gridColor,
        selectionColor: source.selectionColor ?? defaults.selectionColor,
    };
}

function mergeSettings(settings: Partial<AppSettings>): AppSettings {
    return {
        theme: mergeThemeSettings(settings.theme),
        editor: {
            showGrid: settings.editor?.showGrid ?? DEFAULT_EDITOR.showGrid,
            autoSave: settings.editor?.autoSave ?? DEFAULT_EDITOR.autoSave,
            snapToGrid: settings.editor?.snapToGrid ?? DEFAULT_EDITOR.snapToGrid,
            fontSize: settings.editor?.fontSize ?? DEFAULT_EDITOR.fontSize,
            openSideBySideDirection: settings.editor?.openSideBySideDirection ?? DEFAULT_EDITOR.openSideBySideDirection,
            splitOnDragAndDrop: settings.editor?.splitOnDragAndDrop ?? DEFAULT_EDITOR.splitOnDragAndDrop,
            alwaysShowEditorActions: settings.editor?.alwaysShowEditorActions ?? DEFAULT_EDITOR.alwaysShowEditorActions,
            closeEmptyGroups: settings.editor?.closeEmptyGroups ?? DEFAULT_EDITOR.closeEmptyGroups,
            splitSizing: settings.editor?.splitSizing ?? DEFAULT_EDITOR.splitSizing,
        },
        appearance: {
            colorTheme: settings.appearance?.colorTheme ?? DEFAULT_APPEARANCE.colorTheme,
            lastLightColorTheme: settings.appearance?.lastLightColorTheme ?? DEFAULT_APPEARANCE.lastLightColorTheme,
            lastDarkColorTheme: settings.appearance?.lastDarkColorTheme ?? DEFAULT_APPEARANCE.lastDarkColorTheme,
            language: settings.appearance?.language ?? DEFAULT_APPEARANCE.language,

            smoothScroll: settings.appearance?.smoothScroll ?? DEFAULT_APPEARANCE.smoothScroll,
            titleBarStyle: settings.appearance?.titleBarStyle ?? DEFAULT_APPEARANCE.titleBarStyle,
        },
        project: { ...DEFAULT_PROJECT, ...settings.project },
    };
}

function loadLocalSettings(): AppSettings {
    if (typeof localStorage === "undefined") {
        return mergeSettings({});
    }

    try {
        localStorage.removeItem(LEGACY_SETTINGS_STORAGE_KEY);
        const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
        if (!raw) return mergeSettings({});
        return mergeSettings(JSON.parse(raw) as Partial<AppSettings>);
    } catch (error) {
        logger.app.warn(`Failed to load local settings: ${error instanceof Error ? error.message : String(error)}`, "Settings");
        return mergeSettings({});
    }
}

function saveLocalSettings(settings: AppSettings): void {
    if (typeof localStorage === "undefined") return;

    try {
        localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
    } catch (error) {
        logger.app.error(`Failed to save local settings: ${error instanceof Error ? error.message : String(error)}`, "Settings");
        throw error;
    }
}

async function emitClientSettingsUpdated(settings: AppSettings): Promise<void> {
    try {
        await emit(CLIENT_SETTINGS_UPDATED_EVENT, settings);
    } catch {
        // 非 Tauri 环境或事件不可用时忽略
    }
}

function persistClientSettings(settings: AppSettings): void {
    saveLocalSettings(settings);
    if (suppressClientSettingsCrossWindowBroadcast) return;
    void emitClientSettingsUpdated(settings);
}

/**
 * 订阅其他窗口写入的客户端设置；返回取消监听函数。
 * 收到后与当前指纹比对，避免回声与多余渲染。
 */
export async function subscribeClientSettingsCrossWindow(): Promise<() => void> {
    const unlisten = await listen<AppSettings>(CLIENT_SETTINGS_UPDATED_EVENT, (event) => {
        const incoming = event.payload;
        if (!incoming || typeof incoming !== "object") return;

        const merged = mergeSettings(incoming as Partial<AppSettings>);
        const cur = useSettingsStore.getState();
        const currentPayload: AppSettings = {
            theme: cur.theme,
            editor: cur.editor,
            appearance: cur.appearance,
            project: cur.project,
        };
        if (clientSettingsFingerprint(currentPayload) === clientSettingsFingerprint(merged)) {
            return;
        }

        suppressClientSettingsCrossWindowBroadcast = true;
        try {
            saveLocalSettings(merged);
            useSettingsStore.setState({
                theme: merged.theme,
                editor: merged.editor,
                appearance: merged.appearance,
                project: merged.project,
                isLoading: false,
            });
        } finally {
            suppressClientSettingsCrossWindowBroadcast = false;
        }
    });
    return unlisten;
}

interface SettingsStore {
    theme: ThemeSettings;
    editor: EditorSettings;
    appearance: AppearanceSettings;
    project: ProjectSettings;
    isLoading: boolean;

    load: () => Promise<void>;

    // 更新方法（仅更新状态，不保存）
    updateTheme: (updates: Partial<ThemeSettings>) => void;
    updateEditor: (updates: Partial<EditorSettings>) => void;
    updateAppearance: (updates: Partial<AppearanceSettings>) => void;
    updateProject: (updates: Partial<ProjectSettings>) => void;

    // 保存方法
    save: () => Promise<void>;

    // 恢复默认方法
    resetThemeToDefaults: () => Promise<void>;
    resetEditorToDefaults: () => Promise<void>;
    resetAppearanceToDefaults: () => Promise<void>;
    resetProjectToDefaults: () => Promise<void>;

    // 重新加载设置
    resetAllToDefaults: () => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>((set, get) => {
    let saveTimer: ReturnType<typeof setTimeout> | null = null;

    const saveImmediately = async () => {
        // 取消任何待处理的防抖保存
        if (saveTimer) {
            clearTimeout(saveTimer);
            saveTimer = null;
        }
        const state = get();
        const settings: AppSettings = {
            theme: state.theme,
            editor: state.editor,
            appearance: state.appearance,
            project: state.project,
        };
        persistClientSettings(settings);
    };

    const scheduleSave = () => {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            saveImmediately().catch((e) => logger.app.error(String(e), 'Settings'));
        }, 500);
    };

    return {
        theme: DEFAULT_THEME,
        editor: DEFAULT_EDITOR,
        appearance: DEFAULT_APPEARANCE,
        project: DEFAULT_PROJECT,
        isLoading: true,

        load: async () => {
            set({ isLoading: true });
            set({
                ...loadLocalSettings(),
                isLoading: false,
            });
        },

        updateTheme: (updates) =>
            set((state) => {
                const next = { theme: { ...state.theme, ...updates } };
                queueMicrotask(scheduleSave);
                return next;
            }),

        updateEditor: (updates) =>
            set((state) => {
                const next = { editor: { ...state.editor, ...updates } };
                queueMicrotask(scheduleSave);
                return next;
            }),

        updateAppearance: (updates) =>
            set((state) => {
                const next = { appearance: { ...state.appearance, ...updates } };
                queueMicrotask(scheduleSave);
                return next;
            }),

        updateProject: (updates) =>
            set((state) => {
                const next = { project: { ...state.project, ...updates } };
                queueMicrotask(scheduleSave);
                return next;
            }),

        // 立即保存当前状态
        save: saveImmediately,

        // 防抖保存

        resetThemeToDefaults: async () => {
            set({ theme: DEFAULT_THEME });
            await saveImmediately();
        },

        resetEditorToDefaults: async () => {
            set({ editor: DEFAULT_EDITOR });
            await saveImmediately();
        },

        resetAppearanceToDefaults: async () => {
            set({ appearance: DEFAULT_APPEARANCE });
            await saveImmediately();
        },

        resetProjectToDefaults: async () => {
            set({ project: DEFAULT_PROJECT });
            await saveImmediately();
        },

        resetAllToDefaults: async () => {
            const defaults = mergeSettings({});
            set(defaults);
            persistClientSettings(defaults);
        },
    };
});
