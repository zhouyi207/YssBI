// @vitest-environment happy-dom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_AI, DEFAULT_APPEARANCE } from "@/shared/config-default";
import { COLOR_THEME_PRESETS } from "@/shared/theme/colorThemePresets";
import type { AppSettings } from "@/shared/types/settings";
import { SETTINGS_CHANGED_EVENT } from "@/services/platform/settingsEvents";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { settingsRead } from "@/features/core/settings/read";
import { SettingsSyncCoordinator } from "./settingsSyncCoordinator";

const mocks = vi.hoisted(() => ({
  emit: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => mocks);

describe("SettingsSyncCoordinator", () => {
  let coordinator: SettingsSyncCoordinator;
  let receiveSettings: ((event: { payload: unknown }) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    receiveSettings = undefined;
    useSettingsStore.setState({
      ai: DEFAULT_AI,
      appearance: DEFAULT_APPEARANCE,
      isLoading: true,
    });
    mocks.emit.mockResolvedValue(undefined);
    mocks.listen.mockImplementation(
      async (_event: string, listener: (event: { payload: unknown }) => void) => {
        receiveSettings = listener;
        return vi.fn();
      },
    );
    coordinator = new SettingsSyncCoordinator();
  });

  afterEach(() => coordinator.stop());

  it("publishes local settings and applies remote settings without echoing them", async () => {
    await coordinator.start();

    await useSettingsStore.getState().save();
    expect(mocks.emit).toHaveBeenCalledWith(SETTINGS_CHANGED_EVENT, {
      ai: DEFAULT_AI,
      appearance: DEFAULT_APPEARANCE,
    });

    const remote: AppSettings = {
      ai: DEFAULT_AI,
      appearance: {
        ...DEFAULT_APPEARANCE,
        colorTheme: "OLED Black",
        lastDarkColorTheme: "OLED Black",
      },
    };
    receiveSettings?.({ payload: remote });

    const snapshot = settingsRead.getSnapshot();
    expect(snapshot.theme).toBe(COLOR_THEME_PRESETS["OLED Black"]);
    expect(snapshot.appearance).toEqual(remote.appearance);
    expect(mocks.emit).toHaveBeenCalledOnce();

    receiveSettings?.({ payload: remote });
    expect(settingsRead.getSnapshot()).toBe(snapshot);

    receiveSettings?.({ payload: { ...remote, theme: { accentColor: "#123456" } } });
    expect(settingsRead.getSnapshot()).toBe(snapshot);
    expect(mocks.emit).toHaveBeenCalledOnce();
  });
});
