// @vitest-environment happy-dom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_APPEARANCE } from "@/shared/config-default";
import {
  COLOR_THEME_PRESETS,
  DEFAULT_DARK_THEME,
  DEFAULT_LIGHT_THEME,
  getRememberedColorTheme,
} from "@/shared/theme/colorThemePresets";
import {
  applyClientSettingsFromRemote,
  setClientSettingsPublisher,
  useSettingsStore,
} from "./settingsStore";
import { getSettingsSnapshot, subscribeSettingsRead } from "./read";
import { settingsUi } from "./ui";

vi.mock("@/utils/frontendLogger", () => ({
  logger: {
    app: {
      warn: vi.fn(),
      error: vi.fn(),
    },
  },
}));

const SETTINGS_STORAGE_KEY = "yssbi-client-settings-v2";

const APPEARANCE_KEYS = [
  "colorTheme",
  "language",
  "lastDarkColorTheme",
  "lastLightColorTheme",
  "smoothScroll",
  "titleBarStyle",
];

describe("settingsStore appearance persistence", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
    setClientSettingsPublisher(null);
    useSettingsStore.setState({
      appearance: DEFAULT_APPEARANCE,
      isLoading: true,
    });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it("reloads synchronously in one publication and preserves unchanged settings branches", async () => {
    await useSettingsStore.getState().load();
    const before = getSettingsSnapshot();
    const observations: ReturnType<typeof getSettingsSnapshot>[] = [];
    const listener = vi.fn(() => observations.push(getSettingsSnapshot()));
    const unsubscribe = subscribeSettingsRead(listener);
    try {
      localStorage.setItem(
        SETTINGS_STORAGE_KEY,
        JSON.stringify({ appearance: { language: "en-US" } }),
      );
      await useSettingsStore.getState().load();
      expect(listener).toHaveBeenCalledOnce();
      expect(observations.map((snapshot) => snapshot.isLoading)).toEqual([false]);
      expect(getSettingsSnapshot().appearance.language).toBe("en-US");
      expect(getSettingsSnapshot().theme).toBe(before.theme);
      expect(before.appearance.language).toBe(DEFAULT_APPEARANCE.language);

      const loaded = getSettingsSnapshot();
      await useSettingsStore.getState().load();
      useSettingsStore.getState().updateAppearance({ language: "en-US" });
      await useSettingsStore.getState().save();
      expect(listener).toHaveBeenCalledOnce();
      expect(getSettingsSnapshot()).toBe(loaded);
    } finally {
      unsubscribe();
    }
  });

  it("keeps local pending fields and untouched branches when receiving remote settings", async () => {
    await useSettingsStore.getState().load();
    useSettingsStore.getState().updateAppearance({ smoothScroll: false });
    const pending = getSettingsSnapshot();
    const publisher = vi.fn();
    setClientSettingsPublisher(publisher);
    const listener = vi.fn();
    const unsubscribe = subscribeSettingsRead(listener);
    try {
      applyClientSettingsFromRemote({
        appearance: { language: "en-US", smoothScroll: true },
      });
      const remote = getSettingsSnapshot();
      expect(remote.theme).toBe(pending.theme);
      expect(remote.appearance.smoothScroll).toBe(false);
      expect(remote.appearance.language).toBe("en-US");
      expect(pending.appearance.language).toBe(DEFAULT_APPEARANCE.language);
      expect(listener).toHaveBeenCalledOnce();
      expect(publisher).not.toHaveBeenCalled();

      await useSettingsStore.getState().save();
      expect(getSettingsSnapshot()).toBe(remote);
      expect(listener).toHaveBeenCalledOnce();
      expect(publisher).toHaveBeenCalledOnce();
      expect(publisher.mock.calls[0][0].appearance).toEqual({ smoothScroll: false });
      expect(JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY)!).appearance.language).toBe(
        "en-US",
      );
      expect(JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY)!).appearance.smoothScroll).toBe(
        false,
      );
    } finally {
      unsubscribe();
    }
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
    expect(getSettingsSnapshot().theme).toEqual(COLOR_THEME_PRESETS["OLED Black"]);
    await useSettingsStore.getState().save();

    const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}");
    expect(Object.keys(saved).sort()).toEqual(["appearance"]);
    expect(saved.appearance.colorTheme).toBe("OLED Black");
  });

  it("keeps provider credentials out of client preference storage", async () => {
    localStorage.setItem(
      SETTINGS_STORAGE_KEY,
      JSON.stringify({ ai: { openAiApiKey: "secret" }, appearance: { language: "en-US" } }),
    );
    await useSettingsStore.getState().load();
    expect(getSettingsSnapshot().appearance.language).toBe("en-US");
    expect(JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY)!)).not.toHaveProperty("ai");
  });

  it("switches and remembers presets atomically, reuses palettes, and resets via appearance", async () => {
    const listener = vi.fn();
    const unsubscribe = subscribeSettingsRead(listener);
    try {
      settingsUi.updateAppearance({ colorTheme: "OLED Black" });
      expect(listener).toHaveBeenCalledOnce();
      expect(getSettingsSnapshot().theme).toBe(COLOR_THEME_PRESETS["OLED Black"]);
      expect(getSettingsSnapshot().appearance.lastDarkColorTheme).toBe("OLED Black");

      settingsUi.updateAppearance({ colorTheme: "Light Modern" });
      expect(listener).toHaveBeenCalledTimes(2);
      expect(getSettingsSnapshot().theme).toBe(DEFAULT_LIGHT_THEME);
      const { lastLightColorTheme, lastDarkColorTheme } = getSettingsSnapshot().appearance;
      expect(lastLightColorTheme).toBe("Light Modern");
      settingsUi.updateAppearance({
        colorTheme: getRememberedColorTheme("dark", lastLightColorTheme, lastDarkColorTheme),
      });
      const palette = getSettingsSnapshot().theme;
      expect(palette).toBe(COLOR_THEME_PRESETS["OLED Black"]);

      settingsUi.updateAppearance({ smoothScroll: false });
      expect(getSettingsSnapshot().theme).toBe(palette);
      expect(Object.isFrozen(palette)).toBe(true);

      await settingsUi.resetAppearanceToDefaults();
      expect(getSettingsSnapshot().theme).toBe(DEFAULT_DARK_THEME);
      expect(getSettingsSnapshot().appearance).toEqual(DEFAULT_APPEARANCE);

      settingsUi.updateAppearance({ colorTheme: "Light Modern" });
      await settingsUi.resetAllToDefaults();
      expect(getSettingsSnapshot().theme).toBe(DEFAULT_DARK_THEME);
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
