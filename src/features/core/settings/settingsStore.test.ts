// @vitest-environment happy-dom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_AI, DEFAULT_APPEARANCE } from "@/shared/config-default";
import {
  COLOR_THEME_PRESETS,
  DEFAULT_DARK_THEME,
  DEFAULT_LIGHT_THEME,
  getRememberedColorTheme,
} from "@/shared/theme/colorThemePresets";
import { setClientSettingsPublisher, useSettingsStore } from "./settingsStore";
import { settingsRead } from "./read";
import { settingsUi } from "./ui";

vi.mock("@/features/application/observability/appLogger", () => ({
  logger: {
    app: {
      warn: vi.fn(),
      error: vi.fn(),
    },
  },
}));

const SETTINGS_STORAGE_KEY = "yssbi-client-settings-v2";
const LEGACY_SETTINGS_STORAGE_KEY = "yssbi-client-settings";

const APPEARANCE_KEYS = [
  "colorTheme",
  "language",
  "lastDarkColorTheme",
  "lastLightColorTheme",
  "smoothScroll",
  "titleBarStyle",
];

const AI_KEYS = ["openAiApiKey", "openAiBaseUrl", "openAiModel"];

describe("settingsStore appearance persistence", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
    setClientSettingsPublisher(null);
    useSettingsStore.setState({
      ai: DEFAULT_AI,
      appearance: DEFAULT_APPEARANCE,
      isLoading: true,
    });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it("deletes the legacy settings key without reading its theme values", async () => {
    localStorage.setItem(
      LEGACY_SETTINGS_STORAGE_KEY,
      JSON.stringify({
        theme: { accentColor: "#000000" },
      }),
    );

    await useSettingsStore.getState().load();

    expect(localStorage.getItem(LEGACY_SETTINGS_STORAGE_KEY)).toBeNull();
    expect(settingsRead.getSnapshot().theme).toBe(DEFAULT_DARK_THEME);
  });

  it("ignores stored color overrides and persists only the selected theme", async () => {
    localStorage.setItem(
      SETTINGS_STORAGE_KEY,
      JSON.stringify({
        theme: { ...DEFAULT_LIGHT_THEME, accentColor: "#ff0000" },
        appearance: { colorTheme: "OLED Black" },
      }),
    );
    await useSettingsStore.getState().load();
    expect(settingsRead.getSnapshot().theme).toEqual(COLOR_THEME_PRESETS["OLED Black"]);
    await useSettingsStore.getState().save();

    const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}");
    expect(Object.keys(saved).sort()).toEqual(["ai", "appearance"]);
    expect(saved.appearance.colorTheme).toBe("OLED Black");
  });

  it("persists the OpenAI model and API key settings", async () => {
    localStorage.setItem(
      SETTINGS_STORAGE_KEY,
      JSON.stringify({
        ai: {
          openAiModel: "gpt-test",
          openAiBaseUrl: "https://example.test/v1",
          openAiApiKey: "sk-test",
        },
      }),
    );

    await useSettingsStore.getState().load();

    expect(useSettingsStore.getState().ai).toEqual({
      openAiApiKey: "sk-test",
      openAiBaseUrl: "https://example.test/v1",
      openAiModel: "gpt-test",
    });
    await useSettingsStore.getState().save();

    const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}") as {
      ai?: Record<string, unknown>;
    };
    expect(Object.keys(saved.ai ?? {}).sort()).toEqual([...AI_KEYS].sort());
  });

  it("switches and remembers presets atomically, reuses palettes, and resets via appearance", async () => {
    const listener = vi.fn();
    const unsubscribe = settingsRead.subscribe(listener);
    try {
      settingsUi.setTheme("OLED Black");
      expect(listener).toHaveBeenCalledOnce();
      expect(settingsRead.getSnapshot().theme).toBe(COLOR_THEME_PRESETS["OLED Black"]);
      expect(settingsRead.getSnapshot().appearance.lastDarkColorTheme).toBe("OLED Black");

      settingsUi.setTheme("Light Modern");
      expect(listener).toHaveBeenCalledTimes(2);
      expect(settingsRead.getSnapshot().theme).toBe(DEFAULT_LIGHT_THEME);
      const { lastLightColorTheme, lastDarkColorTheme } = settingsRead.getSnapshot().appearance;
      expect(lastLightColorTheme).toBe("Light Modern");
      settingsUi.setTheme(getRememberedColorTheme("dark", lastLightColorTheme, lastDarkColorTheme));
      const palette = settingsRead.getSnapshot().theme;
      expect(palette).toBe(COLOR_THEME_PRESETS["OLED Black"]);

      settingsUi.updateAi({ openAiModel: "gpt-test" });
      expect(settingsRead.getSnapshot().theme).toBe(palette);
      expect(Object.isFrozen(palette)).toBe(true);

      await settingsUi.resetAppearanceToDefaults();
      expect(settingsRead.getSnapshot().theme).toBe(DEFAULT_DARK_THEME);
      expect(settingsRead.getSnapshot().appearance).toEqual(DEFAULT_APPEARANCE);
      expect(settingsRead.getSnapshot().ai.openAiModel).toBe("gpt-test");

      settingsUi.setTheme("Light Modern");
      await settingsUi.resetAllToDefaults();
      expect(settingsRead.getSnapshot().theme).toBe(DEFAULT_DARK_THEME);
      expect(settingsRead.getSnapshot().ai).toEqual(DEFAULT_AI);
    } finally {
      unsubscribe();
    }
  });

  it("projects known appearance fields and removes stored panelPosition on save", async () => {
    localStorage.setItem(
      SETTINGS_STORAGE_KEY,
      JSON.stringify({
        appearance: {
          colorTheme: "OLED Black",
          language: "en-US",
          activityBarPosition: "Right",
          smoothScroll: false,
          titleBarStyle: "native",
          panelPosition: "Left",
          futureAppearanceField: "discard me",
        },
      }),
    );

    await useSettingsStore.getState().load();

    const loaded = useSettingsStore.getState().appearance as unknown as Record<string, unknown>;
    expect(Object.keys(loaded).sort()).toEqual(APPEARANCE_KEYS);
    expect(loaded).not.toHaveProperty("panelPosition");
    expect(loaded).not.toHaveProperty("futureAppearanceField");
    expect(loaded.lastLightColorTheme).toBe(DEFAULT_APPEARANCE.lastLightColorTheme);
    expect(loaded.lastDarkColorTheme).toBe("OLED Black");

    await useSettingsStore.getState().save();

    const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}") as {
      appearance?: Record<string, unknown>;
    };
    expect(Object.keys(saved.appearance ?? {}).sort()).toEqual(APPEARANCE_KEYS);
    expect(saved.appearance).not.toHaveProperty("panelPosition");
  });
});
