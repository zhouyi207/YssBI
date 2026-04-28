import { ThemeSettings, EditorSettings, AppearanceSettings, ProjectSettings, WindowSettings, AppSettings } from "@/shared/types/settings";

export const DEFAULT_DARK_THEME: ThemeSettings = {
    mode: "dark",
    workbenchBackground: "#121212",
    sidebarBackground: "#252526",
    accentColor: "#0078d4",
    gridLines: "#252525",
    nodeBase: "#2d2d2d",
    connectionLines: "#6b6b6b",
    selectionRegion: "#0078d4",
    execColor: "#ffffff",
    int32Color: "#35b2b2",
    int64Color: "#2d9d9d",
    float32Color: "#9ecd4d",
    float64Color: "#8ebd45",
    boolColor: "#e06c75",
    stringColor: "#e5c07b",
    dateColor: "#c678dd",
    datetimeColor: "#c678dd",
    categoricalColor: "#4ec9b0",
    dataframeColor: "#61afef",
    dataseriesColor: "#56b6c2",
    objectColor: "#abb2bf",
    anyColor: "#858585",
    oneofColor: "#7aabc4",
    arrayColor: "#d19a66",
    structColor: "#b07cd8",
};

export const DEFAULT_LIGHT_THEME: ThemeSettings = {
    ...DEFAULT_DARK_THEME,
    mode: "light",
    workbenchBackground: "#f6f7f9",
    sidebarBackground: "#ffffff",
    gridLines: "#e4e7ec",
    nodeBase: "#ffffff",
    connectionLines: "#667085",
    selectionRegion: "#2563eb",
    execColor: "#111827",
    objectColor: "#4b5563",
    anyColor: "#6b7280",
};

export const DEFAULT_THEME: ThemeSettings = DEFAULT_DARK_THEME;

export const DEFAULT_EDITOR: EditorSettings = {
    showGrid: true,
    autoSave: true,
    snapToGrid: true,
    fontSize: 12,
};

export const DEFAULT_APPEARANCE: AppearanceSettings = {
    colorTheme: "Dark Modern (Default)",
    activityBarPosition: "Left",
    smoothScroll: true,
};

export const DEFAULT_PROJECT: ProjectSettings = {
    projectName: "YssBI Project",
    exportPath: "",
};

export const DEFAULT_WINDOW: WindowSettings = {
    width: 1600,
    height: 900,
    x: null,
    y: null,
    isMaximized: false,
};

export const DEFAULT_SETTINGS: AppSettings = {
    theme: DEFAULT_THEME,
    editor: DEFAULT_EDITOR,
    appearance: DEFAULT_APPEARANCE,
    project: DEFAULT_PROJECT,
    window: DEFAULT_WINDOW,
};

export const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

export const GRID = 40;