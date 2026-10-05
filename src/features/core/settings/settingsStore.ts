import { create } from "zustand";
import { shallow } from "zustand/shallow";
import { parseSettingsPatch } from "@/shared/types/settings/parseSettings";
import type { AppearanceSettings, AppSettings, PartialAppSettings } from "@/shared/types/settings";
import { DEFAULT_APPEARANCE } from "@/shared/config-default";
import { getThemeModeForPreset } from "@/shared/theme/colorThemePresets";
import { logger } from "@/utils/frontendLogger";

const SETTINGS_STORAGE_KEY = "yssbi-client-settings-v2";

let pendingSettings: PartialAppSettings = {};
let publishClientSettings: ((settings: PartialAppSettings) => void) | null = null;

export function setClientSettingsPublisher(
  publisher: ((settings: PartialAppSettings) => void) | null,
): void {
  publishClientSettings = publisher;
}

function mergeAppearanceSettings(source?: Partial<AppearanceSettings>): AppearanceSettings {
  const settings = source ?? {};
  const colorTheme = settings.colorTheme ?? DEFAULT_APPEARANCE.colorTheme;
  const isLight = getThemeModeForPreset(colorTheme) === "light";
  return {
    colorTheme,
    // Remember the active preset in the same update, without a follow-up effect/write.
    lastLightColorTheme: isLight
      ? colorTheme
      : (settings.lastLightColorTheme ?? DEFAULT_APPEARANCE.lastLightColorTheme),
    lastDarkColorTheme: isLight
      ? (settings.lastDarkColorTheme ?? DEFAULT_APPEARANCE.lastDarkColorTheme)
      : colorTheme,
    language: settings.language ?? DEFAULT_APPEARANCE.language,
    smoothScroll: settings.smoothScroll ?? DEFAULT_APPEARANCE.smoothScroll,
    titleBarStyle: settings.titleBarStyle ?? DEFAULT_APPEARANCE.titleBarStyle,
  };
}

function mergeSettings(settings: PartialAppSettings): AppSettings {
  return {
    appearance: mergeAppearanceSettings(settings.appearance),
  };
}

function loadLocalSettings(): AppSettings {
  if (typeof localStorage === "undefined") {
    return mergeSettings({});
  }

  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) return mergeSettings({});
    const parsed = parseSettingsPatch(JSON.parse(raw));
    if (!parsed) throw new Error("Invalid settings fields");
    const settings = mergeSettings(parsed);
    if (raw !== JSON.stringify(settings)) saveLocalSettings(settings);
    return settings;
  } catch {
    logger.app.warn(
      "Failed to load local settings: invalid or unavailable stored settings",
      "Settings",
    );
    return mergeSettings({});
  }
}

function saveLocalSettings(settings: AppSettings): void {
  if (typeof localStorage === "undefined") return;

  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch (error) {
    logger.app.error(
      `Failed to save local settings: ${error instanceof Error ? error.message : String(error)}`,
      "Settings",
    );
    throw error;
  }
}

function mergePatches(base: PartialAppSettings, patch: PartialAppSettings): PartialAppSettings {
  return {
    appearance: { ...base.appearance, ...patch.appearance },
  };
}

/** Remote changes cannot overwrite locally pending fields and are never echoed. */
export function applyClientSettingsFromRemote(incoming: PartialAppSettings): void {
  const patch = parseSettingsPatch(incoming);
  if (!patch) return;
  const current = useSettingsStore.getState();
  const merged = mergeSettings(mergePatches(mergePatches(current, patch), pendingSettings));
  if (shallow(current.appearance, merged.appearance)) return;
  saveLocalSettings(mergeSettings(mergePatches(loadLocalSettings(), patch)));
  useSettingsStore.setState((state) => settingsUpdate(state, merged, false));
}

interface SettingsStore {
  appearance: AppearanceSettings;
  isLoading: boolean;

  load: () => Promise<void>;

  // 更新后安排持久化。
  updateAppearance: (updates: Partial<AppearanceSettings>) => void;

  // 保存方法
  save: () => Promise<void>;

  // 恢复默认方法
  resetAppearanceToDefaults: () => Promise<void>;

  // 重新加载设置
  resetAllToDefaults: () => Promise<void>;
}

function settingsUpdate(
  current: SettingsStore,
  next: AppSettings,
  isLoading = current.isLoading,
): Partial<SettingsStore> {
  const appearance = shallow(current.appearance, next.appearance)
    ? current.appearance
    : next.appearance;
  if (appearance === current.appearance && isLoading === current.isLoading) return current;
  return { appearance, isLoading };
}

export const useSettingsStore = create<SettingsStore>((set, get) => {
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  const installSettings = (settings: AppSettings, isLoading?: boolean) =>
    set((current) => settingsUpdate(current, settings, isLoading));

  const saveImmediately = async () => {
    // 取消任何待处理的防抖保存
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    const patch = pendingSettings;
    const settings = mergeSettings(mergePatches(loadLocalSettings(), patch));
    saveLocalSettings(settings);
    pendingSettings = {};
    installSettings(settings);
    if (Object.keys(patch.appearance ?? {}).length) publishClientSettings?.(patch);
  };

  const scheduleSave = () => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveImmediately().catch((e) => logger.app.error(String(e), "Settings"));
    }, 500);
  };

  return {
    appearance: DEFAULT_APPEARANCE,
    isLoading: true,

    load: async () => {
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = null;
      pendingSettings = {};
      installSettings(loadLocalSettings(), false);
    },

    updateAppearance: (updates) => {
      const parsed = parseSettingsPatch({ appearance: updates });
      if (!parsed) throw new Error("Invalid appearance settings fields");
      const current = get().appearance;
      const next = mergeAppearanceSettings({ ...current, ...parsed.appearance });
      const patch = { ...parsed.appearance };
      // A theme switch also changes its remembered preset in the same commit.
      if (next.lastLightColorTheme !== current.lastLightColorTheme)
        patch.lastLightColorTheme = next.lastLightColorTheme;
      if (next.lastDarkColorTheme !== current.lastDarkColorTheme)
        patch.lastDarkColorTheme = next.lastDarkColorTheme;
      pendingSettings = mergePatches(pendingSettings, { appearance: patch });
      installSettings({ appearance: next });
      scheduleSave();
    },

    // 立即保存当前状态
    save: saveImmediately,

    resetAppearanceToDefaults: async () => {
      pendingSettings = mergePatches(pendingSettings, { appearance: DEFAULT_APPEARANCE });
      installSettings({ appearance: DEFAULT_APPEARANCE });
      await saveImmediately();
    },

    resetAllToDefaults: async () => {
      const defaults = mergeSettings({});
      pendingSettings = defaults;
      installSettings(defaults);
      await saveImmediately();
    },
  };
});
