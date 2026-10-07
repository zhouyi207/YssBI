export type ThemeMode = "light" | "dark";

export interface ThemePalette {
  readonly mode: ThemeMode;
  readonly workbenchBackground: string;
  readonly sidebarBackground: string;
  readonly nodeBackground: string;
  readonly foreground: string;
  readonly mutedForeground: string;
  readonly accentColor: string;
  readonly borderColor: string;
  readonly gridColor: string;
  readonly selectionColor: string;
}

export const DEFAULT_DARK_THEME: ThemePalette = Object.freeze({
  mode: "dark",
  // Analytical instrument palette: deep slate surfaces with one cobalt interaction signal.
  workbenchBackground: "#11151c",
  sidebarBackground: "#171d27",
  nodeBackground: "#1c2430",
  foreground: "#e7ebf3",
  mutedForeground: "#929db0",
  accentColor: "#5b82f6",
  borderColor: "#334155",
  gridColor: "#2a3444",
  selectionColor: "#5b82f6",
});

export const DEFAULT_LIGHT_THEME: ThemePalette = Object.freeze({
  ...DEFAULT_DARK_THEME,
  mode: "light",
  workbenchBackground: "#f5f7fa",
  sidebarBackground: "#edf1f6",
  nodeBackground: "#ffffff",
  foreground: "#202938",
  mutedForeground: "#596579",
  accentColor: "#315ede",
  borderColor: "#d7dee9",
  gridColor: "#d9e1ec",
  selectionColor: "#315ede",
});

export const COLOR_THEME_PRESET_IDS = [
  "Dark Modern (Default)",
  "OLED Black",
  "Light Modern",
] as const;

export type ColorThemePresetId = (typeof COLOR_THEME_PRESET_IDS)[number];

export function getColorThemeForMode(mode: ThemeMode): ColorThemePresetId {
  return mode === "light" ? "Light Modern" : "Dark Modern (Default)";
}

export function getRememberedColorTheme(
  mode: ThemeMode,
  rememberedLight: string,
  rememberedDark: string,
): string {
  return (mode === "light" ? rememberedLight : rememberedDark) || getColorThemeForMode(mode);
}

export function getThemeModeForPreset(colorTheme: string): ThemeMode {
  return resolveColorThemePreset(colorTheme).mode;
}

export const COLOR_THEME_PRESETS: Readonly<Record<ColorThemePresetId, ThemePalette>> =
  Object.freeze({
    "Dark Modern (Default)": DEFAULT_DARK_THEME,
    "OLED Black": Object.freeze({
      ...DEFAULT_DARK_THEME,
      workbenchBackground: "#000000",
      sidebarBackground: "#000000",
      nodeBackground: "#0a0a0a",
      gridColor: "#141414",
    }),
    "Light Modern": DEFAULT_LIGHT_THEME,
  });

export function resolveColorThemePreset(colorTheme: string): ThemePalette {
  if (Object.prototype.hasOwnProperty.call(COLOR_THEME_PRESETS, colorTheme)) {
    return COLOR_THEME_PRESETS[colorTheme as ColorThemePresetId];
  }
  return COLOR_THEME_PRESETS["Dark Modern (Default)"];
}
