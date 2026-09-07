import type { AppearanceSettings, AiSettings } from "@/shared/types/settings";

export const DEFAULT_AI: AiSettings = {
  openAiApiKey: "",
  openAiBaseUrl: "https://api.openai.com/v1",
  openAiModel: "",
};

export const DEFAULT_APPEARANCE: AppearanceSettings = {
  colorTheme: "Dark Modern (Default)",
  lastLightColorTheme: "Light Modern",
  lastDarkColorTheme: "Dark Modern (Default)",
  language: "zh-CN",

  smoothScroll: true,
  titleBarStyle: "custom",
};

export const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

export const GRID = 40;
