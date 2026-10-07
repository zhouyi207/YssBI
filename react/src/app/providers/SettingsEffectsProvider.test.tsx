// @vitest-environment happy-dom

import { describe, expect, it } from "vitest";
import { COLOR_THEME_PRESET_IDS, resolveColorThemePreset } from "@/shared/theme/colorThemePresets";
import { resolveThemeTokens } from "@/shared/theme/themeTokens";
import { applyThemeTokens } from "./SettingsEffectsProvider";

describe("applyThemeTokens", () => {
  const root = document.createElement("html");
  it.each(COLOR_THEME_PRESET_IDS)("applies the %s preset to all CSS theme variables", (preset) => {
    const tokens = resolveThemeTokens(resolveColorThemePreset(preset));

    applyThemeTokens(root, tokens);

    expect(root.classList.contains("dark")).toBe(tokens.mode === "dark");
    expect(root.style.colorScheme).toBe(tokens.mode);
    expect(root.style.getPropertyValue("--background")).toBe(tokens.workbenchBg);
    expect(root.style.getPropertyValue("--foreground")).toBe(tokens.foreground);
    expect(root.style.getPropertyValue("--muted")).toBe(tokens.surfaceSunken);
    expect(root.style.getPropertyValue("--secondary")).toBe(tokens.surfaceRaised);
    expect(root.style.getPropertyValue("--primary")).toBe(tokens.accent);
    expect(root.style.getPropertyValue("--primary-foreground")).toBe(tokens.primaryForeground);
    expect(root.style.getPropertyValue("--border")).toBe(tokens.border);
    expect(root.style.getPropertyValue("--grid-lines")).toBe(tokens.grid);
    expect(root.style.getPropertyValue("--selection-region")).toBe(tokens.selection);
    expect(root.style.getPropertyValue("--pin-table")).toBe(tokens.pins.table);
  });
});
