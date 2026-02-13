/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { NodeDefinition } from "@/shared/types/editor";
import { NodeDefinitionMap, NodeRegistryState } from "./nodeRegistry.types";
import { LoadStatus } from "@/shared/types/loadStatus";

interface NodeRegistryStore extends NodeRegistryState {
    definitions: NodeDefinitionMap;

    syncFromBackend: () => Promise<void>;
    clear: () => void;

    getDefinition: (type: string) => NodeDefinition | undefined;
    getAllDefinitions: () => NodeDefinition[];
    hasDefinition: (type: string) => boolean;
}

export const useNodeRegistryStore = create<NodeRegistryStore>((set, get) => ({
    // data
    definitions: new Map(),

    // state (来自 NodeRegistryState)
    status: LoadStatus.Idle,
    error: null,

    syncFromBackend: async () => {
        const { status } = get();

        // 幂等保护
        if (status === LoadStatus.Loading || status === LoadStatus.Ready) {
            console.log('[NodeRegistry] Already loading or loaded, skipping...');
            return;
        }

        const startTime = performance.now();
        console.log('[NodeRegistry] Loading node definitions from backend...');

        set({ status: LoadStatus.Loading, error: null });

        try {
            const defs = await invoke<NodeDefinition[]>("get_node_definitions");

            const definitions = new Map<string, NodeDefinition>();
            defs.forEach((def) => definitions.set(def.node_type, def));

            set({
                definitions,
                status: LoadStatus.Ready,
            });

            const duration = performance.now() - startTime;
            console.log('[NodeRegistry] ✓ Node definitions loaded successfully', {
                nodeTypes: definitions.size,
                duration: `${duration.toFixed(0)}ms`,
            });
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            console.error('[NodeRegistry] ✗ Failed to load node definitions:', errorMessage);
            
            set({
                status: LoadStatus.Error,
                error: errorMessage,
            });
            
            throw err;
        }
    },

    clear: () =>
        set({
            definitions: new Map(),
            status: LoadStatus.Idle,
            error: null,
        }),

    getDefinition: (type) => get().definitions.get(type),
    getAllDefinitions: () => Array.from(get().definitions.values()),
    hasDefinition: (type) => get().definitions.has(type),
}));
