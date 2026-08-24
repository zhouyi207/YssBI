import { ThemeSettings, EditorSettings, AppearanceSettings, ProjectSettings, AppSettings } from "@/shared/types/settings";

export const DEFAULT_DARK_THEME: ThemeSettings = {
    mode: "dark",
    // Analytical instrument palette: deep slate surfaces with one cobalt interaction signal.
    workbenchBackground: "#11151c",
    sidebarBackground: "#171d27",
    accentColor: "#5b82f6",
    gridLines: "#2a3444",
    nodeBase: "#1c2430",
    connectionLines: "#77849a",
    selectionRegion: "#5b82f6",
    execColor: "#ffffff",
    int32Color: "#5eead4",
    int64Color: "#2dd4bf",
    float32Color: "#a3e635",
    float64Color: "#84cc16",
    boolColor: "#fb7185",
    stringColor: "#fbbf24",
    dateColor: "#c084fc",
    datetimeColor: "#a78bfa",
    categoricalColor: "#34d399",
    dataframeColor: "#60a5fa",
    dataseriesColor: "#22d3ee",
    objectColor: "#d4d4d4",
    anyColor: "#a3a3a3",
    oneofColor: "#7dd3fc",
    arrayColor: "#fb923c",
    structColor: "#c084fc",
};

export const DEFAULT_LIGHT_THEME: ThemeSettings = {
    ...DEFAULT_DARK_THEME,
    mode: "light",
    workbenchBackground: "#f5f7fa",
    sidebarBackground: "#edf1f6",
    accentColor: "#315ede",
    gridLines: "#d9e1ec",
    nodeBase: "#ffffff",
    connectionLines: "#667085",
    selectionRegion: "#315ede",
    execColor: "#111827",
    int32Color: "#0f766e",
    int64Color: "#0e7490",
    float32Color: "#4d7c0f",
    float64Color: "#3f6212",
    boolColor: "#b91c1c",
    stringColor: "#a16207",
    dateColor: "#7e22ce",
    datetimeColor: "#7e22ce",
    categoricalColor: "#047857",
    dataframeColor: "#1d4ed8",
    dataseriesColor: "#0369a1",
    objectColor: "#475569",
    anyColor: "#64748b",
    oneofColor: "#2563eb",
    arrayColor: "#c2410c",
    structColor: "#7c3aed",
};

export const DEFAULT_THEME: ThemeSettings = DEFAULT_DARK_THEME;

export const DEFAULT_EDITOR: EditorSettings = {
    showGrid: true,
    autoSave: true,
    snapToGrid: true,
    fontSize: 12,
    openSideBySideDirection: 'right',
    splitOnDragAndDrop: true,
    alwaysShowEditorActions: false,
    closeEmptyGroups: true,
    splitSizing: 'auto',
};

export const DEFAULT_APPEARANCE: AppearanceSettings = {
    colorTheme: "Dark Modern (Default)",
    lastLightColorTheme: "Light Modern",
    lastDarkColorTheme: "Dark Modern (Default)",
    language: "zh-CN",
    activityBarPosition: "Left",
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
