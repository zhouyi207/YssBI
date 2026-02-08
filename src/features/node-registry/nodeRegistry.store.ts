/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { NodeDefinition } from "@/views/EditorView/Types/nodes";
import { NodeDefinitionMap } from "./nodeRegistry.types";

interface NodeRegistryStore {
    definitions: NodeDefinitionMap;
    isInitialized: boolean;
    isLoading: boolean;
    error: string | null;

    syncFromBackend: () => Promise<void>;
    clear: () => void;

    getDefinition: (type: string) => NodeDefinition | undefined;
    getAllDefinitions: () => NodeDefinition[];
    hasDefinition: (type: string) => boolean;
}

export const useNodeRegistryStore = create<NodeRegistryStore>((set, get) => ({
    definitions: new Map(),
    isInitialized: false,
    isLoading: false,
    error: null,

    syncFromBackend: async () => {
        set({ isLoading: true, error: null });

        try {
            const defs = await invoke<NodeDefinition[]>("get_node_definitions");

            const definitions = new Map<string, NodeDefinition>();
            defs.forEach((def) => definitions.set(def.node_type, def));

            set({
                definitions,
                isInitialized: true,
                isLoading: false,
            });
        } catch (err) {
            set({
                isLoading: false,
                error: err instanceof Error ? err.message : String(err),
            });
            throw err;
        }
    },

    clear: () =>
        set({
            definitions: new Map(),
            isInitialized: false,
            error: null,
        }),

    getDefinition: (type) => get().definitions.get(type),
    getAllDefinitions: () => Array.from(get().definitions.values()),
    hasDefinition: (type) => get().definitions.has(type),
}));
