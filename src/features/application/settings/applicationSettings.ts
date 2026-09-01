import { useSettingsRead } from "@/features/core/settings/read";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { settingsUi } from "@/features/core/settings/ui";

const loadSettings = () => useSettingsStore.getState().load();

export function useApplicationThemeMode() {
  return useSettingsRead((state) => state.theme.mode);
}

export function useApplicationAppearance() {
  const themeMode = useSettingsRead((state) => state.theme.mode ?? "dark");
  const appearance = useSettingsRead((state) => state.appearance);
  return {
    themeMode,
    appearance,
    updateAppearance: settingsUi.updateAppearance,
  };
}

/** Settings read/actions needed by the application composition effects. */
export function useApplicationSettings() {
  const snapshot = useSettingsRead((state) => state);
  return {
    ...snapshot,
    load: loadSettings,
    updateTheme: settingsUi.updateTheme,
    updateAppearance: settingsUi.updateAppearance,
  };
}
