import { describe, expect, it } from "vitest";
import { DEFAULT_DARK_THEME } from "@/shared/config-default";
import { getPinPalette, getReadableForeground, resolveThemeTokens } from "./themeTokens";

describe("theme token resolver", () => {
  it("chooses readable foreground for dark and light accents", () => {
    expect(getReadableForeground("#111827")).toBe("#ffffff");
    expect(getReadableForeground("#f8fafc")).toBe("#0d1524");
  });

  it("derives surfaces and interaction tokens from semantic settings", () => {
    const tokens = resolveThemeTokens({
      ...DEFAULT_DARK_THEME,
      workbenchBackground: "#080b12",
      sidebarBackground: "#101827",
      nodeBackground: "#172235",
      accentColor: "#f8fafc",
      borderColor: "#30415d",
      gridColor: "#1d2a40",
      selectionColor: "#38bdf8",
    });

    expect(tokens.workbenchBg).toBe("#080b12");
    expect(tokens.sidebarBg).toBe("#101827");
    expect(tokens.nodeForeground).toBe("#ffffff");
    expect(tokens.primaryForeground).toBe("#0d1524");
    expect(tokens.border).toBe("#30415d");
    expect(tokens.grid).toBe("#1d2a40");
    expect(tokens.selection).toBe("#38bdf8");
    expect(tokens.surfaceRaised).toContain("color-mix");
  });

  it("falls back to the default accent when a setting contains an invalid color", () => {
    const tokens = resolveThemeTokens({ ...DEFAULT_DARK_THEME, accentColor: "not-a-color" });

    expect(tokens.accent).toBe(DEFAULT_DARK_THEME.accentColor);
    expect(tokens.primaryForeground).toBe("#ffffff");
  });

  it("provides stable semantic pin categories without per-pin settings", () => {
    const palette = getPinPalette("dark");

    expect(Object.keys(palette).sort()).toEqual([
      "boolean",
      "numeric",
      "object",
      "table",
      "temporal",
      "text",
    ]);
    expect(palette.numeric).toBe("#5eead4");
  });
});
