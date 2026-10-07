import type { ThemeMode } from "./colorThemePresets";
export function resolveYssbiLayoutTheme(mode: ThemeMode): string {
  return mode === "light" ? "flexlayout__theme_alpha_light" : "flexlayout__theme_alpha_dark";
}
