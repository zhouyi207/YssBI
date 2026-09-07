import { create } from "zustand";
import type {
  AiSettings,
  AppearanceSettings,
  AppSettings,
  PartialAppSettings,
} from "@/shared/types/settings";
import { DEFAULT_APPEARANCE, DEFAULT_AI } from "@/shared/config-default";
import { getThemeModeForPreset } from "@/shared/theme/colorThemePresets";
import { logger } from "@/features/core/observability/logger";

const SETTINGS_STORAGE_KEY = "yssbi-client-settings-v2";
const LEGACY_SETTINGS_STORAGE_KEY = "yssbi-client-settings";

let suppressClientSettingsCrossWindowBroadcast = false;
let publishClientSettings: ((settings: AppSettings) => void) | null = null;

export function setClientSettingsPublisher(
  publisher: ((settings: AppSettings) => void) | null,
): void {
  publishClientSettings = publisher;
}

function clientSettingsFingerprint(s: AppSettings): string {
  return JSON.stringify({
    ai: s.ai,
    appearance: s.appearance,
  });
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
    ai: {
      openAiApiKey: settings.ai?.openAiApiKey ?? DEFAULT_AI.openAiApiKey,
      openAiBaseUrl: settings.ai?.openAiBaseUrl ?? DEFAULT_AI.openAiBaseUrl,
      openAiModel: settings.ai?.openAiModel ?? DEFAULT_AI.openAiModel,
    },
    appearance: mergeAppearanceSettings(settings.appearance),
  };
}

function loadLocalSettings(): AppSettings {
  if (typeof localStorage === "undefined") {
    return mergeSettings({});
  }

  try {
    localStorage.removeItem(LEGACY_SETTINGS_STORAGE_KEY);
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) return mergeSettings({});
    return mergeSettings(JSON.parse(raw) as PartialAppSettings);
  } catch (error) {
    logger.app.warn(
      `Failed to load local settings: ${error instanceof Error ? error.message : String(error)}`,
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

function persistClientSettings(settings: AppSettings): void {
  saveLocalSettings(settings);
  if (suppressClientSettingsCrossWindowBroadcast) return;
  publishClientSettings?.(settings);
}

/** 应用其他窗口写入的客户端设置，避免相同快照回声与多余渲染。 */
export function applyClientSettingsFromRemote(incoming: AppSettings): void {
  const merged = mergeSettings(incoming);
  const cur = useSettingsStore.getState();
  const currentPayload: AppSettings = {
    ai: cur.ai,
    appearance: cur.appearance,
  };
  if (clientSettingsFingerprint(currentPayload) === clientSettingsFingerprint(merged)) {
    return;
  }

  suppressClientSettingsCrossWindowBroadcast = true;
  try {
    saveLocalSettings(merged);
    useSettingsStore.setState({
      ai: merged.ai,
      appearance: merged.appearance,
      isLoading: false,
    });
  } finally {
    suppressClientSettingsCrossWindowBroadcast = false;
  }
}

interface SettingsStore {
  ai: AiSettings;
  appearance: AppearanceSettings;
  isLoading: boolean;

  load: () => Promise<void>;

  // 更新后安排持久化。
  updateAi: (updates: Partial<AiSettings>) => void;
  updateAppearance: (updates: Partial<AppearanceSettings>) => void;

  // 保存方法
  save: () => Promise<void>;

  // 恢复默认方法
  resetAiToDefaults: () => Promise<void>;
  resetAppearanceToDefaults: () => Promise<void>;

  // 重新加载设置
  resetAllToDefaults: () => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>((set, get) => {
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  const saveImmediately = async () => {
    // 取消任何待处理的防抖保存
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    const state = get();
    const settings: AppSettings = {
      ai: state.ai,
      appearance: state.appearance,
    };
    persistClientSettings(settings);
  };

  const scheduleSave = () => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveImmediately().catch((e) => logger.app.error(String(e), "Settings"));
    }, 500);
  };

  return {
    ai: DEFAULT_AI,
    appearance: DEFAULT_APPEARANCE,
    isLoading: true,

    load: async () => {
      set({ isLoading: true });
      set({
        ...loadLocalSettings(),
        isLoading: false,
      });
    },

    updateAi: (updates) =>
      set((state) => {
        const next = { ai: { ...state.ai, ...updates } };
        queueMicrotask(scheduleSave);
        return next;
      }),

    updateAppearance: (updates) =>
      set((state) => {
        const next = { appearance: mergeAppearanceSettings({ ...state.appearance, ...updates }) };
        queueMicrotask(scheduleSave);
        return next;
      }),

    // 立即保存当前状态
    save: saveImmediately,

    resetAiToDefaults: async () => {
      set({ ai: DEFAULT_AI });
      await saveImmediately();
    },

    resetAppearanceToDefaults: async () => {
      set({ appearance: DEFAULT_APPEARANCE });
      await saveImmediately();
    },

    resetAllToDefaults: async () => {
      const defaults = mergeSettings({});
      set(defaults);
      persistClientSettings(defaults);
    },
  };
});
