import { describe, expect, it } from "vitest";
import { DEFAULT_DARK_THEME } from "./colorThemePresets";
import { getReadableForeground, resolveThemeTokens } from "./themeTokens";

describe("theme token resolver", () => {
  it("chooses readable foreground for dark and light accents", () => {
    expect(getReadableForeground("#111827")).toBe("#ffffff");
    expect(getReadableForeground("#f8fafc")).toBe("#0d1524");
  });

  it("falls back to the default accent when a palette contains an invalid color", () => {
    const tokens = resolveThemeTokens({ ...DEFAULT_DARK_THEME, accentColor: "not-a-color" });

    expect(tokens.accent).toBe(DEFAULT_DARK_THEME.accentColor);
    expect(tokens.primaryForeground).toBe("#ffffff");
  });
});
