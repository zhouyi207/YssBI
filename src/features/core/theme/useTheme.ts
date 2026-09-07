import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { resolveColorThemePreset } from "@/shared/theme/colorThemePresets";
import { resolveThemeTokens } from "@/shared/theme/themeTokens";
import { useMemo } from "react";

export const useTheme = () => {
  const theme = useSettingsStore((s) => resolveColorThemePreset(s.appearance.colorTheme));
  const tokens = useMemo(() => resolveThemeTokens(theme), [theme]);

  return { theme, tokens };
};
