import { DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME } from "@/shared/config-default";
import type { ThemeSettings } from "@/shared/types/settings";

const THEME_BASE_KEYS = [
  "mode",
  "workbenchBackground",
  "sidebarBackground",
  "nodeBackground",
  "foreground",
  "mutedForeground",
  "accentColor",
  "borderColor",
  "gridColor",
  "selectionColor",
] satisfies Array<keyof ThemeSettings>;

function pickThemeBase(theme: ThemeSettings): Partial<ThemeSettings> {
  return Object.fromEntries(
    THEME_BASE_KEYS.map((key) => [key, theme[key]]),
  ) as Partial<ThemeSettings>;
}

export const COLOR_THEME_PRESET_IDS = [
  "Dark Modern (Default)",
  "OLED Black",
  "Light Modern",
] as const;

export type ColorThemePresetId = (typeof COLOR_THEME_PRESET_IDS)[number];

export function getColorThemeForMode(mode: "light" | "dark"): ColorThemePresetId {
  return mode === "light" ? "Light Modern" : "Dark Modern (Default)";
}

export function getRememberedColorTheme(
  mode: "light" | "dark",
  rememberedLight: string,
  rememberedDark: string,
): string {
  return (mode === "light" ? rememberedLight : rememberedDark) || getColorThemeForMode(mode);
}

export function getThemeModeForPreset(colorTheme: string): "light" | "dark" {
  return colorTheme === "Light Modern" ? "light" : "dark";
}

export const COLOR_THEME_PRESETS: Record<ColorThemePresetId, Partial<ThemeSettings>> = {
  "Dark Modern (Default)": pickThemeBase(DEFAULT_DARK_THEME),
  "OLED Black": {
    ...pickThemeBase(DEFAULT_DARK_THEME),
    workbenchBackground: "#000000",
    sidebarBackground: "#000000",
    nodeBackground: "#0a0a0a",
    gridColor: "#141414",
  },
  "Light Modern": pickThemeBase(DEFAULT_LIGHT_THEME),
};

export function resolveColorThemePreset(colorTheme: string): Partial<ThemeSettings> {
  if (colorTheme in COLOR_THEME_PRESETS) {
    return COLOR_THEME_PRESETS[colorTheme as ColorThemePresetId];
  }
  return COLOR_THEME_PRESETS["Dark Modern (Default)"];
}
