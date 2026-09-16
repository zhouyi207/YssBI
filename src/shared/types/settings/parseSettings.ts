import type { AiSettings, AppearanceSettings, PartialAppSettings } from "./index";

type Validators<T> = { [K in keyof T]-?: (value: unknown) => boolean };
const isString = (value: unknown) => typeof value === "string";
const fields = {
  ai: {
    openAiApiKey: isString,
    openAiBaseUrl: isString,
    openAiModel: isString,
  } satisfies Validators<AiSettings>,
  appearance: {
    colorTheme: isString,
    lastLightColorTheme: isString,
    lastDarkColorTheme: isString,
    language: (value: unknown) => value === "zh-CN" || value === "en-US",
    smoothScroll: (value: unknown) => typeof value === "boolean",
    titleBarStyle: (value: unknown) => value === "custom" || value === "native",
  } satisfies Validators<AppearanceSettings>,
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Project known fields; missing fields stay absent, malformed known fields reject the input. */
export function parseSettingsPatch(value: unknown): PartialAppSettings | null {
  if (!isRecord(value)) return null;
  const result: Record<string, Record<string, unknown>> = {};
  for (const section of ["ai", "appearance"] as const) {
    if (!Object.prototype.hasOwnProperty.call(value, section)) continue;
    const input = value[section];
    if (!isRecord(input)) return null;
    const parsed: Record<string, unknown> = {};
    for (const [field, accepts] of Object.entries(fields[section])) {
      if (!Object.prototype.hasOwnProperty.call(input, field)) continue;
      if (!accepts(input[field])) return null;
      parsed[field] = input[field];
    }
    result[section] = parsed;
  }
  return result as PartialAppSettings;
}
