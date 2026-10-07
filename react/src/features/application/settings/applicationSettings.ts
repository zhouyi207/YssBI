import { getSettingsSnapshot, useSettingsRead } from "@/features/core/settings/read";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { settingsUi } from "@/features/core/settings/ui";
import { getRememberedColorTheme } from "@/shared/theme/colorThemePresets";

const loadSettings = () => useSettingsStore.getState().load();

export function useApplicationThemeMode() {
  return useSettingsRead((state) => state.theme.mode);
}

export function useApplicationTheme() {
  return useSettingsRead((state) => state.theme);
}

export function toggleApplicationThemeMode(): void {
  const { theme, appearance } = getSettingsSnapshot();
  settingsUi.updateAppearance({
    colorTheme: getRememberedColorTheme(
      theme.mode === "light" ? "dark" : "light",
      appearance.lastLightColorTheme,
      appearance.lastDarkColorTheme,
    ),
  });
}

/** Settings read/actions needed by the application composition effects. */
export function useApplicationSettings() {
  const theme = useApplicationTheme();
  const language = useSettingsRead((state) => state.appearance.language);
  const smoothScroll = useSettingsRead((state) => state.appearance.smoothScroll);
  return {
    theme,
    language,
    smoothScroll,
    load: loadSettings,
  };
}
