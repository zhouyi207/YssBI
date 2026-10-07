import { z } from "zod";

export const REASONING_EFFORTS = ["low", "medium", "high"] as const;
export const reasoningEffortSchema = z.enum(REASONING_EFFORTS);
export type ReasoningEffort = z.infer<typeof reasoningEffortSchema>;

export const languageModelSelectionSchema = z.strictObject({
  providerId: z.string().min(1),
  modelId: z.string().min(1),
});
export const languageModelIdentitySchema = z.strictObject({
  selection: languageModelSelectionSchema,
  // The configuration's display name captured when the turn starts.
  providerName: z.string().min(1),
  modelName: z.string().min(1),
});
export const languageModelConfigSchema = z.strictObject({
  reasoningEfforts: z.array(reasoningEffortSchema).optional(),
  id: z.string().min(1),
  name: z.string().min(1),
  contextWindow: z.number().int().positive().nullable(),
  maxOutputTokens: z.number().int().positive().nullable(),
  temperature: z.number().min(0).max(2).nullable(),
  topP: z.number().min(0).max(1).nullable(),
  additionalParameters: z.record(z.string(), z.json()),
});
export const languageModelProtocolSchema = z.enum([
  "open_ai_responses",
  "open_ai_chat",
  "anthropic",
  "gemini",
]);
const authenticationSchema = z.enum(["api_key", "none"]);
export const languageModelProviderSchema = z.strictObject({
  id: z.string().min(1),
  name: z.string().min(1),
  customName: z.string().nullable(),
  protocol: languageModelProtocolSchema,
  adapter: z.string().min(1),
  authentication: authenticationSchema,
  baseUrl: z.string().min(1),
  models: z.array(languageModelConfigSchema),
});
export const languageModelCatalogSchema = z.strictObject({
  presets: z.array(
    z.strictObject({
      id: z.string().min(1),
      name: z.string().min(1),
      protocol: languageModelProtocolSchema,
      adapter: z.string().min(1),
      authentication: authenticationSchema,
      baseUrl: z.string(),
    }),
  ),
  providers: z.array(
    z.strictObject({
      config: languageModelProviderSchema,
      hasApiKey: z.boolean(),
      reasoningDefaults: z.record(z.string(), reasoningEffortSchema).optional(),
    }),
  ),
  defaultModel: languageModelSelectionSchema.nullable(),
});
export type LanguageModelSelection = z.infer<typeof languageModelSelectionSchema>;
export type LanguageModelIdentity = z.infer<typeof languageModelIdentitySchema>;
export type LanguageModelConfig = z.infer<typeof languageModelConfigSchema>;
export type LanguageModelProvider = z.infer<typeof languageModelProviderSchema>;
export type LanguageModelCatalog = z.infer<typeof languageModelCatalogSchema>;

/** An absent or empty override does not require users to opt in to effort selection. */
export function modelReasoningEfforts(
  model: LanguageModelConfig | undefined,
  defaultEffort: ReasoningEffort | null = null,
) {
  if (!model) return undefined;
  const efforts = model.reasoningEfforts?.length ? model.reasoningEfforts : REASONING_EFFORTS;
  return defaultEffort && !efforts.includes(defaultEffort) ? [...efforts, defaultEffort] : efforts;
}

export function providerDisplayName(
  provider: Pick<LanguageModelProvider, "name" | "customName">,
): string {
  return provider.customName?.trim() || provider.name.trim();
}

export function modelSelectionKey(model: LanguageModelSelection | null): string {
  return model ? JSON.stringify([model.providerId, model.modelId]) : "";
}

export function isModelConfigured(
  catalog: LanguageModelCatalog | null,
  selection: LanguageModelSelection | null,
): boolean {
  return (
    !!selection &&
    !!catalog?.providers.some(
      ({ config, hasApiKey }) =>
        (config.authentication === "none" || hasApiKey) &&
        config.id === selection.providerId &&
        config.models.some((model) => model.id === selection.modelId),
    )
  );
}

export function emptyLanguageModel(id = ""): LanguageModelConfig {
  return {
    id,
    name: id,
    contextWindow: null,
    maxOutputTokens: null,
    temperature: null,
    topP: null,
    additionalParameters: {},
  };
}
