import { create } from "zustand";
import {
    ThemeSettings,
    EditorSettings,
    AppearanceSettings,
    ProjectSettings,
    WindowSettings,
    AppSettings,
} from "@/shared/types/settings";
import {
    DEFAULT_THEME,
    DEFAULT_EDITOR,
    DEFAULT_APPEARANCE,
    DEFAULT_WINDOW,
    DEFAULT_PROJECT,
} from "@/app/appConfig/default";
import { SettingsService } from "@/services/settings";
import { logger } from '@/utils/appLogger';

interface SettingsStore {
    theme: ThemeSettings;
    editor: EditorSettings;
    appearance: AppearanceSettings;
    project: ProjectSettings;
    window: WindowSettings;
    isLoading: boolean;

    load: () => Promise<void>;

    // 更新方法（仅更新状态，不保存）
    updateTheme: (updates: Partial<ThemeSettings>) => void;
    updateEditor: (updates: Partial<EditorSettings>) => void;
    updateAppearance: (updates: Partial<AppearanceSettings>) => void;
    updateProject: (updates: Partial<ProjectSettings>) => void;
    updateWindow: (updates: Partial<WindowSettings>) => void;

    // 保存方法
    save: () => Promise<void>;
    saveDebounced: () => void;

    // 恢复默认方法
    resetThemeToDefaults: () => Promise<void>;
    resetEditorToDefaults: () => Promise<void>;
    resetAppearanceToDefaults: () => Promise<void>;
    resetProjectToDefaults: () => Promise<void>;
    resetWindowToDefaults: () => Promise<void>;

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
            window: state.window,
        };
        await SettingsService.saveSettings(settings);
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
        window: DEFAULT_WINDOW,
        isLoading: true,

        load: async () => {
            set({ isLoading: true });
            const settings = await SettingsService.loadSettings();
            set({
                ...settings,
                theme: { ...DEFAULT_THEME, ...settings.theme },
                appearance: { ...DEFAULT_APPEARANCE, ...settings.appearance },
                isLoading: false,
            });
        },

        // 仅更新状态，不保存
        updateTheme: (updates) =>
            set((state) => ({
                theme: { ...state.theme, ...updates }
            })),

        updateEditor: (updates) =>
            set((state) => ({
                editor: { ...state.editor, ...updates }
            })),

        updateAppearance: (updates) =>
            set((state) => ({
                appearance: { ...state.appearance, ...updates }
            })),

        updateProject: (updates) =>
            set((state) => ({
                project: { ...state.project, ...updates }
            })),

        updateWindow: (updates) =>
            set((state) => ({
                window: { ...state.window, ...updates }
            })),

        // 立即保存当前状态
        save: saveImmediately,

        // 防抖保存
        saveDebounced: scheduleSave,

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

        resetWindowToDefaults: async () => {
            set({ window: DEFAULT_WINDOW });
            await saveImmediately();
        },

        resetAllToDefaults: async () => {
            const defaults = await SettingsService.resetToDefaults();
            set(defaults);
        },
    };
});
