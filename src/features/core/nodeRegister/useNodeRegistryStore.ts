/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { SchemaService } from "@/services/schema";
import { NodeDefinition } from "@/shared/types/domain";
import { NodeDefinitionMap, NodeRegistryState } from "@/shared/types/state";
import { LoadStatus } from "@/shared/types/ui";
import { logger } from '@/utils/appLogger';

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
            logger.sys.debug('Already loading or loaded, skipping...', 'NodeRegistry');
            return;
        }

        const startTime = performance.now();
        logger.sys.debug('Loading node definitions from backend...', 'NodeRegistry');

        set({ status: LoadStatus.Loading, error: null });

        try {
            const defs = await SchemaService.getNodeDefinition();

            const definitions = new Map<string, NodeDefinition>();
            defs.forEach((def) => {
                definitions.set(def.nodeType, def);
            });

            // 缓存数组，避免每次 getAllDefinitions 都创建新引用
            const definitionsArray = Array.from(definitions.values());

            set({
                definitions,
                definitionsArray,
                status: LoadStatus.Ready,
            });

            const duration = performance.now() - startTime;
            logger.sys.info(`Node definitions loaded successfully, nodeTypes: ${definitions.size}, duration: ${duration.toFixed(0)}ms`, 'NodeRegistry');
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            logger.sys.error('Failed to load node definitions: ' + errorMessage, 'NodeRegistry');
            
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
