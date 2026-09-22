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
