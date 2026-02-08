import { create } from "zustand";
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
import { SettingsService } from "@/services/settings";

interface SettingsStore {
    theme: ThemeSettings;
    editor: EditorSettings;
    appearance: AppearanceSettings;
    project: ProjectSettings;
    isLoading: boolean;

    load: () => Promise<void>;

    // 更新方法
    updateTheme: (updates: Partial<ThemeSettings>) => void;
    updateEditor: (updates: Partial<EditorSettings>) => void;
    updateAppearance: (updates: Partial<AppearanceSettings>) => void;
    updateProject: (updates: Partial<ProjectSettings>) => void;

    // 恢复默认方法
    resetThemeToDefaults: () => Promise<void>;
    resetEditorToDefaults: () => Promise<void>;
    resetAppearanceToDefaults: () => Promise<void>;
    resetProjectToDefaults: () => Promise<void>;

    // 重新加载设置
    resetAll: () => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>((set) => {
    let saveTimer: ReturnType<typeof setTimeout> | null = null;

    const saveDebounced = (settings: AppSettings) => {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            SettingsService.saveSettings(settings).catch(console.error);
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
            const settings = await SettingsService.loadSettings();
            set({
                ...settings,
                isLoading: false,
            });
        },

        updateTheme: (updates) =>
            set((state) => {
                const theme = { ...state.theme, ...updates };
                saveDebounced({ ...SettingsService.getCachedSettings()!, theme });
                return { theme };
            }),

        updateEditor: (updates) =>
            set((state) => {
                const editor = { ...state.editor, ...updates };
                saveDebounced({ ...SettingsService.getCachedSettings()!, editor });
                return { editor };
            }),

        updateAppearance: (updates) =>
            set((state) => {
                const appearance = { ...state.appearance, ...updates };
                saveDebounced({ ...SettingsService.getCachedSettings()!, appearance });
                return { appearance };
            }),

        updateProject: (updates) =>
            set((state) => {
                const project = { ...state.project, ...updates };
                saveDebounced({ ...SettingsService.getCachedSettings()!, project });
                return { project };
            }),


        resetThemeToDefaults: async () => {
            set({ theme: DEFAULT_THEME });
            const settings = SettingsService.getCachedSettings();
            if (settings) {
                await SettingsService.saveSettings({ ...settings, theme: DEFAULT_THEME });
            }
        },

        resetEditorToDefaults: async () => {
            set({ editor: DEFAULT_EDITOR });
            const settings = SettingsService.getCachedSettings();
            if (settings) {
                await SettingsService.saveSettings({ ...settings, editor: DEFAULT_EDITOR });
            }
        },

        resetAppearanceToDefaults: async () => {
            set({ appearance: DEFAULT_APPEARANCE });
            const settings = SettingsService.getCachedSettings();
            if (settings) {
                await SettingsService.saveSettings({ ...settings, appearance: DEFAULT_APPEARANCE });
            }
        },

        resetProjectToDefaults: async () => {
            set({ project: DEFAULT_PROJECT });
            const settings = SettingsService.getCachedSettings();
            if (settings) {
                await SettingsService.saveSettings({ ...settings, project: DEFAULT_PROJECT });
            }
        },

        resetAll: async () => {
            const defaults = await SettingsService.resetToDefaults();
            set(defaults);
        },
    };
});
