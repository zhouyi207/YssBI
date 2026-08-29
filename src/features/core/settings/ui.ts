import { useSettingsStore } from './settingsStore';
import type {
  AppearanceSettings,
  EditorSettings,
  ProjectSettings,
  ThemeSettings,
} from '@/shared/types/settings';

export interface SettingsUiCapability {
  readonly setTheme: (theme: string) => void;
  readonly setEditorOption: (key: string, value: string | number | boolean) => void;
  readonly updateTheme: (updates: Partial<ThemeSettings>) => void;
  readonly updateEditor: (updates: Partial<EditorSettings>) => void;
  readonly updateAppearance: (updates: Partial<AppearanceSettings>) => void;
  readonly updateProject: (updates: Partial<ProjectSettings>) => void;
  readonly resetAllToDefaults: () => Promise<void>;
  readonly resetThemeToDefaults: () => Promise<void>;
  readonly resetEditorToDefaults: () => Promise<void>;
  readonly resetAppearanceToDefaults: () => Promise<void>;
  readonly resetProjectToDefaults: () => Promise<void>;
}

export const settingsUi: SettingsUiCapability = {
  setTheme: (theme) => useSettingsStore.getState().updateAppearance({ colorTheme: theme }),
  setEditorOption: (key, value) => {
    useSettingsStore.getState().updateEditor({ [key]: value } as Partial<EditorSettings>);
  },
  updateTheme: (updates) => useSettingsStore.getState().updateTheme(updates),
  updateEditor: (updates) => useSettingsStore.getState().updateEditor(updates),
  updateAppearance: (updates) => useSettingsStore.getState().updateAppearance(updates),
  updateProject: (updates) => useSettingsStore.getState().updateProject(updates),
  resetAllToDefaults: () => useSettingsStore.getState().resetAllToDefaults(),
  resetThemeToDefaults: () => useSettingsStore.getState().resetThemeToDefaults(),
  resetEditorToDefaults: () => useSettingsStore.getState().resetEditorToDefaults(),
  resetAppearanceToDefaults: () => useSettingsStore.getState().resetAppearanceToDefaults(),
  resetProjectToDefaults: () => useSettingsStore.getState().resetProjectToDefaults(),
};
