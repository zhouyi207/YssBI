import type {
  LanguageModelCatalog,
  LanguageModelIdentity,
} from "@/services/assistant/modelContract";

export const testModelIdentity: LanguageModelIdentity = {
  selection: { providerId: "test-provider", modelId: "test-model" },
  providerName: "Work account",
  modelName: "Test model",
};
export const testModelCatalog: LanguageModelCatalog = {
  presets: [],
  defaultModel: testModelIdentity.selection,
  providers: [
    {
      hasApiKey: true,
      config: {
        id: "test-provider",
        name: "Test provider",
        customName: "Work account",
        protocol: "open_ai_chat",
        adapter: "openai/openai",
        authentication: "api_key",
        baseUrl: "https://example.test/v1",
        models: [
          {
            id: "test-model",
            name: "Test model",
            contextWindow: null,
            maxOutputTokens: null,
            temperature: null,
            topP: null,
            additionalParameters: {},
          },
        ],
      },
    },
  ],
};
