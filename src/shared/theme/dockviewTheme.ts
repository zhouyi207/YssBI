import { themeDark, themeLight, type DockviewTheme } from "dockview-react";

import type { ThemeMode } from "@/shared/types/settings";

export const YSSBI_DOCKVIEW_DARK_THEME: DockviewTheme = {
  ...themeDark,
  name: "yssbi-dark",
  className: "dockview-theme-dark yssbi-dockview-theme",
  colorScheme: "dark",
  edgeGroupCollapsedSize: 32,
  tabAnimation: "default",
  dndTabIndicator: "line",
};

export const YSSBI_DOCKVIEW_LIGHT_THEME: DockviewTheme = {
  ...themeLight,
  name: "yssbi-light",
  className: "dockview-theme-light yssbi-dockview-theme",
  colorScheme: "light",
  edgeGroupCollapsedSize: 32,
  tabAnimation: "default",
  dndTabIndicator: "line",
};

export const YSSBI_LOGS_DOCKVIEW_DARK_THEME: DockviewTheme = {
  ...YSSBI_DOCKVIEW_DARK_THEME,
  name: "yssbi-logs-dark",
  edgeGroupCollapsedSize: 30,
};

export const YSSBI_LOGS_DOCKVIEW_LIGHT_THEME: DockviewTheme = {
  ...YSSBI_DOCKVIEW_LIGHT_THEME,
  name: "yssbi-logs-light",
  edgeGroupCollapsedSize: 30,
};

export function resolveYssbiDockviewTheme(mode: ThemeMode): DockviewTheme {
  return mode === "light" ? YSSBI_DOCKVIEW_LIGHT_THEME : YSSBI_DOCKVIEW_DARK_THEME;
}

export function resolveYssbiLogsDockviewTheme(mode: ThemeMode): DockviewTheme {
  return mode === "light" ? YSSBI_LOGS_DOCKVIEW_LIGHT_THEME : YSSBI_LOGS_DOCKVIEW_DARK_THEME;
}
