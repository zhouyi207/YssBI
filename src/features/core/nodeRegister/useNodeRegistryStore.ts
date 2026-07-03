/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { NodeDefinition } from "@/shared/types/domain";
import { NodeDefinitionMap, NodeRegistryState } from "@/shared/types/state";
import { LoadStatus } from "@/shared/types/ui";

interface NodeRegistryStore extends NodeRegistryState {
    definitions: NodeDefinitionMap;
    definitionsArray: NodeDefinition[]; // 缓存的数组，避免每次创建新引用

    /** 由 Schema store 在初始化时调用，从 schema 填充 definitions */
    setDefinitionsFromSchema: (definitions: Map<string, NodeDefinition>) => void;
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

    setDefinitionsFromSchema: (definitions) => {
        const definitionsArray = Array.from(definitions.values());
        set({
            definitions,
            definitionsArray,
            status: LoadStatus.Ready,
            error: null,
        });
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
