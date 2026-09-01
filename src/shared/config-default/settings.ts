import {
  ThemeSettings,
  EditorSettings,
  AppearanceSettings,
  ProjectSettings,
  AppSettings,
} from "@/shared/types/settings";

export const DEFAULT_DARK_THEME: ThemeSettings = {
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
};

export const DEFAULT_LIGHT_THEME: ThemeSettings = {
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
};

export const DEFAULT_THEME: ThemeSettings = DEFAULT_DARK_THEME;

export const DEFAULT_EDITOR: EditorSettings = {
  showGrid: true,
  autoSave: true,
  snapToGrid: true,
  fontSize: 12,
  openSideBySideDirection: "right",
  splitOnDragAndDrop: true,
  alwaysShowEditorActions: false,
  closeEmptyGroups: true,
  splitSizing: "auto",
};

export const DEFAULT_APPEARANCE: AppearanceSettings = {
  colorTheme: "Dark Modern (Default)",
  lastLightColorTheme: "Light Modern",
  lastDarkColorTheme: "Dark Modern (Default)",
  language: "zh-CN",

  smoothScroll: true,
  titleBarStyle: "custom",
};

export const DEFAULT_PROJECT: ProjectSettings = {
  projectName: "YssBI Project",
  exportPath: "",
};

export const DEFAULT_SETTINGS: AppSettings = {
  theme: DEFAULT_THEME,
  editor: DEFAULT_EDITOR,
  appearance: DEFAULT_APPEARANCE,
  project: DEFAULT_PROJECT,
};

export const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

export const GRID = 40;
