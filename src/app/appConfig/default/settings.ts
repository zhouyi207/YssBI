import { ThemeSettings, EditorSettings, AppearanceSettings, ProjectSettings, AppSettings } from "@/shared/types/settings";

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
    workbenchBackground: "#ffffff",
    sidebarBackground: "#f8fafc",
    accentColor: "#2563eb",
    gridLines: "#e5e7eb",
    nodeBase: "#ffffff",
    connectionLines: "#64748b",
    selectionRegion: "#2563eb",
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
    doubleClickTabToToggleEditorGroupSizes: 'maximize',
};

export const DEFAULT_APPEARANCE: AppearanceSettings = {
    colorTheme: "Dark Modern (Default)",
    language: "zh-CN",
    activityBarPosition: "Left",
    panelPosition: "Bottom",
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