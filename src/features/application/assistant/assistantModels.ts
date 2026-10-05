import { useEffect } from "react";
import { useStore } from "zustand";
import { createStore } from "zustand/vanilla";
import { ModelService } from "@/services/assistant/modelService";
import type {
  LanguageModelCatalog,
  LanguageModelProvider,
  LanguageModelSelection,
} from "@/services/assistant/modelContract";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";

interface ModelCatalogSnapshot {
  readonly catalog: LanguageModelCatalog | null;
  readonly loading: boolean;
  readonly saving: boolean;
  readonly error: (ErrorReference & { readonly operation: "load" | "mutation" }) | null;
}

export const assistantModels = createStore<ModelCatalogSnapshot>(() => ({
  catalog: null,
  loading: false,
  saving: false,
  error: null,
}));
let generation = 0;
let pending: Promise<void> | null = null;
const INVALIDATION_KEY = "yssbi.assistant.models.changed";

export function reloadModels(): Promise<void> {
  if (assistantModels.getState().saving) return Promise.resolve();
  if (pending) return pending;
  const request = ++generation;
  assistantModels.setState({ loading: true });
  const loading = ModelService.catalog()
    .then((catalog) => {
      if (request === generation)
        assistantModels.setState({ catalog, error: null, loading: false });
    })
    .catch((error) => {
      if (request === generation)
        assistantModels.setState({
          loading: false,
          error: {
            ...toErrorReference(error, "assistant_model_settings_unavailable"),
            operation: "load",
          },
        });
    })
    .finally(() => {
      if (pending === loading) pending = null;
    });
  pending = loading;
  return loading;
}

async function mutate(action: () => Promise<LanguageModelCatalog>): Promise<boolean> {
  if (assistantModels.getState().saving) return false;
  ++generation;
  pending = null;
  assistantModels.setState({ saving: true, loading: false, error: null });
  try {
    const catalog = await action();
    ++generation;
    pending = null;
    assistantModels.setState({ catalog, saving: false, loading: false, error: null });
    // Invalidation only. Other windows reload authoritative Rust settings.
    try {
      localStorage.setItem(INVALIDATION_KEY, crypto.randomUUID());
    } catch {
      /* Focus also refreshes. */
    }
    return true;
  } catch (error) {
    assistantModels.setState({
      saving: false,
      error: {
        ...toErrorReference(error, "assistant_model_settings_unavailable"),
        operation: "mutation",
      },
    });
    return false;
  }
}

export const modelActions = {
  save: (config: LanguageModelProvider, apiKey: string | null) =>
    mutate(() => ModelService.saveProvider(config, apiKey)),
  remove: (providerId: string) => mutate(() => ModelService.deleteProvider(providerId)),
  setDefault: (model: LanguageModelSelection) => mutate(() => ModelService.setDefault(model)),
  reload: reloadModels,
};

export function useAssistantModels() {
  const snapshot = useStore(assistantModels);
  useEffect(() => {
    void reloadModels();
    const reload = () => {
      void reloadModels();
    };
    const onStorage = (event: StorageEvent) => {
      if (event.key === INVALIDATION_KEY) reload();
    };
    window.addEventListener("focus", reload);
    window.addEventListener("storage", onStorage);
    return () => {
      window.removeEventListener("focus", reload);
      window.removeEventListener("storage", onStorage);
    };
  }, []);
  return snapshot;
}
