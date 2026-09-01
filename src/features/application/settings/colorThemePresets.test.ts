import { describe, expect, it } from "vitest";
import {
  getColorThemeForMode,
  getRememberedColorTheme,
  getThemeModeForPreset,
} from "./colorThemePresets";

describe("color theme presets", () => {
  it("maps header light/dark mode actions to canonical presets", () => {
    expect(getColorThemeForMode("light")).toBe("Light Modern");
    expect(getColorThemeForMode("dark")).toBe("Dark Modern (Default)");
  });

  it("classifies presets by their effective color mode", () => {
    expect(getThemeModeForPreset("Light Modern")).toBe("light");
    expect(getThemeModeForPreset("OLED Black")).toBe("dark");
  });

  it("falls back only when a remembered mode theme is missing", () => {
    expect(getRememberedColorTheme("light", "Light Modern", "Dark Modern (Default)")).toBe(
      "Light Modern",
    );
    expect(getRememberedColorTheme("dark", "", "OLED Black")).toBe("OLED Black");
    expect(getRememberedColorTheme("light", "", "")).toBe("Light Modern");
  });
});
