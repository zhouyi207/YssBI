export type ThemeMode = "light" | "dark";

export interface ThemeSettings {
  mode: ThemeMode;
  workbenchBackground: string;
  sidebarBackground: string;
  nodeBackground: string;
  foreground: string;
  mutedForeground: string;
  accentColor: string;
  borderColor: string;
  gridColor: string;
  selectionColor: string;
}
