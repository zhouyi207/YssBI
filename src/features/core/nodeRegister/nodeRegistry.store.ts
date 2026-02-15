/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { SchemaService } from "@/services/schema";
import { NodeDefinition } from "@/shared/types/domain";
import { NodeDefinitionMap, NodeRegistryState } from "@/shared/types/domain/nodeRegister";
import { LoadStatus } from "@/shared/types/ui";

interface NodeRegistryStore extends NodeRegistryState {
    definitions: NodeDefinitionMap;
    definitionsArray: NodeDefinition[]; // 缓存的数组，避免每次创建新引用

    syncFromBackend: () => Promise<void>;
    clear: () => void;

    getDefinition: (type: string) => NodeDefinition | undefined;
    getAllDefinitions: () => NodeDefinition[];
    hasDefinition: (type: string) => boolean;
}

export const useNodeRegistryStore = create<NodeRegistryStore>((set, get) => ({
    // data
    definitions: new Map(),
    definitionsArray: [],

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
            const defs = await SchemaService.getNodeDefinition();

            const definitions = new Map<string, NodeDefinition>();
            defs.forEach((def) => {
                // 使用完整路径 (category:name) 作为主键
                const fullName = [...def.category, def.name].join(':');
                definitions.set(fullName, def);
                
                // 同时使用简单名称作为别名，方便查找
                // 注意：如果有重名，后面的会覆盖前面的
                definitions.set(def.name, def);
            });

            // 缓存数组，避免每次 getAllDefinitions 都创建新引用
            const definitionsArray = Array.from(definitions.values());

            set({
                definitions,
                definitionsArray,
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
            definitionsArray: [],
            status: LoadStatus.Idle,
            error: null,
        }),

    getDefinition: (type) => get().definitions.get(type),
    getAllDefinitions: () => get().definitionsArray,
    hasDefinition: (type) => get().definitions.has(type),
}));
