import { z } from "zod";
import { invokeCommand } from "@/services/ipc";
import {
  languageModelCatalogSchema,
  languageModelConfigSchema,
  type LanguageModelProvider,
  type LanguageModelSelection,
} from "./modelContract";

export const ModelService = {
  async catalog() {
    return languageModelCatalogSchema.parse(await invokeCommand("list_harness_models"));
  },
  async saveProvider(config: LanguageModelProvider, apiKey: string | null) {
    return languageModelCatalogSchema.parse(
      await invokeCommand("save_harness_provider", { request: { config, apiKey } }),
    );
  },
  async deleteProvider(providerId: string) {
    return languageModelCatalogSchema.parse(
      await invokeCommand("delete_harness_provider", { providerId }),
    );
  },
  async discoverModels(config: LanguageModelProvider, apiKey: string | null) {
    return z.array(languageModelConfigSchema).parse(
      await invokeCommand("discover_harness_models", {
        request: {
          config: {
            ...config,
            name: config.name.trim(),
            baseUrl: config.baseUrl.trim(),
            models: [],
          },
          apiKey,
        },
      }),
    );
  },
  async setDefault(model: LanguageModelSelection) {
    return languageModelCatalogSchema.parse(
      await invokeCommand("set_default_harness_model", { model }),
    );
  },
};
