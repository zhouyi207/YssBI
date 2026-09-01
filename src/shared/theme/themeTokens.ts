import { DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME } from "@/shared/config-default";
import type { ThemeMode, ThemeSettings } from "@/shared/types/settings";

export type PinSemanticCategory =
  | "exec"
  | "numeric"
  | "boolean"
  | "text"
  | "temporal"
  | "table"
  | "object";

export type PinPalette = Record<PinSemanticCategory, string>;

export interface ThemeStatusPalette {
  success: string;
  warning: string;
  danger: string;
  info: string;
}

export interface ResolvedThemeTokens {
  mode: ThemeMode;
  workbenchBg: string;
  sidebarBg: string;
  panelBg: string;
  surfaceRaised: string;
  surfaceSunken: string;
  panelHeaderBg: string;
  nodeBg: string;
  foreground: string;
  mutedForeground: string;
  nodeForeground: string;
  primaryForeground: string;
  accent: string;
  accentHover: string;
  accentSoft: string;
  border: string;
  inputBorder: string;
  ring: string;
  grid: string;
  selection: string;
  connection: string;
  status: ThemeStatusPalette;
  pins: PinPalette;
}

const DARK_PIN_PALETTE: PinPalette = {
  exec: "#ffffff",
  numeric: "#5eead4",
  boolean: "#fb7185",
  text: "#fbbf24",
  temporal: "#c084fc",
  table: "#60a5fa",
  object: "#d4d4d4",
};

const LIGHT_PIN_PALETTE: PinPalette = {
  exec: "#111827",
  numeric: "#0e7490",
  boolean: "#b91c1c",
  text: "#a16207",
  temporal: "#7e22ce",
  table: "#1d4ed8",
  object: "#475569",
};

const DARK_STATUS: ThemeStatusPalette = {
  success: "#34d399",
  warning: "#fbbf24",
  danger: "#f97066",
  info: "#60a5fa",
};

const LIGHT_STATUS: ThemeStatusPalette = {
  success: "#047857",
  warning: "#a16207",
  danger: "#b42318",
  info: "#1d4ed8",
};

function normalizeHex(value: string, fallback: string): string {
  const trimmed = value.trim();
  if (/^#[0-9a-f]{6}$/i.test(trimmed)) return trimmed;
  if (/^#[0-9a-f]{3}$/i.test(trimmed)) {
    return `#${trimmed
      .slice(1)
      .split("")
      .map((part) => `${part}${part}`)
      .join("")}`;
  }
  return fallback;
}

function hexToRgb(value: string): [number, number, number] | null {
  const normalized = normalizeHex(value, "");
  if (!normalized) return null;
  return [
    Number.parseInt(normalized.slice(1, 3), 16),
    Number.parseInt(normalized.slice(3, 5), 16),
    Number.parseInt(normalized.slice(5, 7), 16),
  ];
}

function relativeLuminance(value: string): number | null {
  const rgb = hexToRgb(value);
  if (!rgb) return null;
  const channels = rgb.map((channel) => {
    const normalized = channel / 255;
    return normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

export function getReadableForeground(background: string): string {
  const luminance = relativeLuminance(background);
  if (luminance == null) return "#0d1524";
  return luminance > 0.42 ? "#0d1524" : "#ffffff";
}

function colorMix(base: string, percentage: number, mixColor: string): string {
  return `color-mix(in srgb, ${base} ${percentage}%, ${mixColor})`;
}

export function getPinPalette(mode: ThemeMode): PinPalette {
  return mode === "light" ? { ...LIGHT_PIN_PALETTE } : { ...DARK_PIN_PALETTE };
}

export function resolveThemeTokens(theme: ThemeSettings): ResolvedThemeTokens {
  const defaults = theme.mode === "light" ? DEFAULT_LIGHT_THEME : DEFAULT_DARK_THEME;
  const workbenchBg = normalizeHex(theme.workbenchBackground, defaults.workbenchBackground);
  const sidebarBg = normalizeHex(theme.sidebarBackground, defaults.sidebarBackground);
  const nodeBg = normalizeHex(theme.nodeBackground, defaults.nodeBackground);
  const foreground = normalizeHex(theme.foreground, defaults.foreground);
  const mutedForeground = normalizeHex(theme.mutedForeground, defaults.mutedForeground);
  const accent = normalizeHex(theme.accentColor, defaults.accentColor);
  const border = normalizeHex(theme.borderColor, defaults.borderColor);
  const grid = normalizeHex(theme.gridColor, defaults.gridColor);
  const selection = normalizeHex(theme.selectionColor, defaults.selectionColor);
  const isDark = theme.mode !== "light";

  return {
    mode: theme.mode,
    workbenchBg,
    sidebarBg,
    panelBg: sidebarBg,
    surfaceRaised: isDark
      ? colorMix(sidebarBg, 98, "#ffffff")
      : colorMix(workbenchBg, 35, "#ffffff"),
    surfaceSunken: isDark
      ? colorMix(workbenchBg, 92, "#000000")
      : colorMix(sidebarBg, 82, workbenchBg),
    panelHeaderBg: sidebarBg,
    nodeBg,
    foreground,
    mutedForeground,
    nodeForeground: getReadableForeground(nodeBg),
    primaryForeground: getReadableForeground(accent),
    accent,
    accentHover: colorMix(accent, 84, isDark ? "#ffffff" : "#000000"),
    accentSoft: colorMix(accent, 12, "transparent"),
    border,
    inputBorder: colorMix(border, isDark ? 72 : 86, foreground),
    ring: accent,
    grid,
    selection,
    connection: colorMix(foreground, isDark ? 44 : 52, workbenchBg),
    status: isDark ? { ...DARK_STATUS } : { ...LIGHT_STATUS },
    pins: getPinPalette(theme.mode),
  };
}
