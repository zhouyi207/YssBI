export type ThemeMode = "light" | "dark";

export interface ThemeSettings {
    mode: ThemeMode;
    workbenchBackground: string;
    sidebarBackground: string;
    accentColor: string;
    gridLines: string;
    nodeBase: string;
    connectionLines: string;
    selectionRegion: string;
    // Pin & Type Colors（保留精度：Int32/Int64/Float32/Float64）
    execColor: string;
    int32Color: string;
    int64Color: string;
    float32Color: string;
    float64Color: string;
    boolColor: string;
    stringColor: string;
    dateColor: string;
    datetimeColor: string;
    categoricalColor: string;
    dataframeColor: string;
    dataseriesColor: string;
    objectColor: string;
    anyColor: string;
    oneofColor: string;
    arrayColor: string;
    structColor: string;
}