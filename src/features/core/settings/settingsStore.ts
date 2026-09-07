import { create } from "zustand";
import {
  AiSettings,
  ThemeSettings,
  AppearanceSettings,
  AppSettings,
} from "@/shared/types/settings";
import { DEFAULT_THEME, DEFAULT_APPEARANCE, DEFAULT_AI } from "@/shared/config-default";
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
    theme: s.theme,
    appearance: s.appearance,
  });
}

function mergeThemeSettings(theme: Partial<ThemeSettings> | undefined): ThemeSettings {
  const source = theme ?? {};
  const defaults = DEFAULT_THEME;
  return {
    mode: source.mode === "light" ? "light" : "dark",
    workbenchBackground: source.workbenchBackground ?? defaults.workbenchBackground,
    sidebarBackground: source.sidebarBackground ?? defaults.sidebarBackground,
    nodeBackground: source.nodeBackground ?? defaults.nodeBackground,
    foreground: source.foreground ?? defaults.foreground,
    mutedForeground: source.mutedForeground ?? defaults.mutedForeground,
    accentColor: source.accentColor ?? defaults.accentColor,
    borderColor: source.borderColor ?? defaults.borderColor,
    gridColor: source.gridColor ?? defaults.gridColor,
    selectionColor: source.selectionColor ?? defaults.selectionColor,
  };
}

function mergeSettings(settings: Partial<AppSettings>): AppSettings {
  return {
    ai: {
      openAiApiKey: settings.ai?.openAiApiKey ?? DEFAULT_AI.openAiApiKey,
      openAiBaseUrl: settings.ai?.openAiBaseUrl ?? DEFAULT_AI.openAiBaseUrl,
      openAiModel: settings.ai?.openAiModel ?? DEFAULT_AI.openAiModel,
    },
    theme: mergeThemeSettings(settings.theme),
    appearance: {
      colorTheme: settings.appearance?.colorTheme ?? DEFAULT_APPEARANCE.colorTheme,
      lastLightColorTheme:
        settings.appearance?.lastLightColorTheme ?? DEFAULT_APPEARANCE.lastLightColorTheme,
      lastDarkColorTheme:
        settings.appearance?.lastDarkColorTheme ?? DEFAULT_APPEARANCE.lastDarkColorTheme,
      language: settings.appearance?.language ?? DEFAULT_APPEARANCE.language,

      smoothScroll: settings.appearance?.smoothScroll ?? DEFAULT_APPEARANCE.smoothScroll,
      titleBarStyle: settings.appearance?.titleBarStyle ?? DEFAULT_APPEARANCE.titleBarStyle,
    },
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
    return mergeSettings(JSON.parse(raw) as Partial<AppSettings>);
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
    theme: cur.theme,
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
      theme: merged.theme,
      appearance: merged.appearance,
      isLoading: false,
    });
  } finally {
    suppressClientSettingsCrossWindowBroadcast = false;
  }
}

interface SettingsStore {
  ai: AiSettings;
  theme: ThemeSettings;
  appearance: AppearanceSettings;
  isLoading: boolean;

  load: () => Promise<void>;

  // 更新后安排持久化。
  updateTheme: (updates: Partial<ThemeSettings>) => void;
  updateAi: (updates: Partial<AiSettings>) => void;
  updateAppearance: (updates: Partial<AppearanceSettings>) => void;

  // 保存方法
  save: () => Promise<void>;

  // 恢复默认方法
  resetThemeToDefaults: () => Promise<void>;
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
      theme: state.theme,
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
    theme: DEFAULT_THEME,
    appearance: DEFAULT_APPEARANCE,
    isLoading: true,

    load: async () => {
      set({ isLoading: true });
      set({
        ...loadLocalSettings(),
        isLoading: false,
      });
    },

    updateTheme: (updates) =>
      set((state) => {
        const next = { theme: { ...state.theme, ...updates } };
        queueMicrotask(scheduleSave);
        return next;
      }),

    updateAi: (updates) =>
      set((state) => {
        const next = { ai: { ...state.ai, ...updates } };
        queueMicrotask(scheduleSave);
        return next;
      }),

    updateAppearance: (updates) =>
      set((state) => {
        const next = { appearance: { ...state.appearance, ...updates } };
        queueMicrotask(scheduleSave);
        return next;
      }),

    // 立即保存当前状态
    save: saveImmediately,

    // 防抖保存

    resetThemeToDefaults: async () => {
      set({ theme: DEFAULT_THEME });
      await saveImmediately();
    },

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
