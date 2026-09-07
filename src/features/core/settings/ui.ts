import { useSettingsStore } from "./settingsStore";
import type { AiSettings, AppearanceSettings, ThemeSettings } from "@/shared/types/settings";

export interface SettingsUiCapability {
  readonly setTheme: (theme: string) => void;
  readonly updateTheme: (updates: Partial<ThemeSettings>) => void;
  readonly updateAi: (updates: Partial<AiSettings>) => void;
  readonly updateAppearance: (updates: Partial<AppearanceSettings>) => void;
  readonly resetAllToDefaults: () => Promise<void>;
  readonly resetThemeToDefaults: () => Promise<void>;
  readonly resetAiToDefaults: () => Promise<void>;
  readonly resetAppearanceToDefaults: () => Promise<void>;
}

export const settingsUi: SettingsUiCapability = {
  setTheme: (theme) => useSettingsStore.getState().updateAppearance({ colorTheme: theme }),
  updateTheme: (updates) => useSettingsStore.getState().updateTheme(updates),
  updateAi: (updates) => useSettingsStore.getState().updateAi(updates),
  updateAppearance: (updates) => useSettingsStore.getState().updateAppearance(updates),
  resetAllToDefaults: () => useSettingsStore.getState().resetAllToDefaults(),
  resetThemeToDefaults: () => useSettingsStore.getState().resetThemeToDefaults(),
  resetAiToDefaults: () => useSettingsStore.getState().resetAiToDefaults(),
  resetAppearanceToDefaults: () => useSettingsStore.getState().resetAppearanceToDefaults(),
};
