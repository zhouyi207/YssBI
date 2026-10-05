import { useSettingsStore } from "./settingsStore";
import type { AppearanceSettings } from "@/shared/types/settings";

export interface SettingsUiCapability {
  readonly updateAppearance: (updates: Partial<AppearanceSettings>) => void;
  readonly resetAllToDefaults: () => Promise<void>;
  readonly resetAppearanceToDefaults: () => Promise<void>;
}

export const settingsUi: SettingsUiCapability = {
  updateAppearance: (updates) => useSettingsStore.getState().updateAppearance(updates),
  resetAllToDefaults: () => useSettingsStore.getState().resetAllToDefaults(),
  resetAppearanceToDefaults: () => useSettingsStore.getState().resetAppearanceToDefaults(),
};
