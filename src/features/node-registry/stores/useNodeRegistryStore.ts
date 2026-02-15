import { create } from "zustand";
import { SchemaService } from "@/services/schema";
import { NodeDefinition } from "@/shared/types/editor";

interface NodeRegistryStore {
  // 状态
  definitions: Map<string, NodeDefinition>;
  definitionsArray: NodeDefinition[]; // 缓存的数组
  isInitialized: boolean;
  isLoading: boolean;
  error: string | null;

  // 操作
  syncFromBackend: () => Promise<void>;
  clear: () => void;

  // 查询方法
  getDefinition: (type: string) => NodeDefinition | undefined;
  getAllDefinitions: () => NodeDefinition[];
  hasDefinition: (type: string) => boolean;
}

export const useNodeRegistryStore = create<NodeRegistryStore>((set, get) => ({
  // 初始状态
  definitions: new Map(),
  definitionsArray: [],
  isInitialized: false,
  isLoading: false,
  error: null,

  // 从后端同步节点定义
  syncFromBackend: async () => {
    set({ isLoading: true, error: null });

    try {
      const defs = await SchemaService.getNodeDefinition();


      const definitions = new Map<string, NodeDefinition>();
      defs.forEach(def => {
        definitions.set(def.name, def);
      });

      // 同时更新缓存的数组
      const definitionsArray = Array.from(definitions.values());

      set({
        definitions,
        definitionsArray,
        isInitialized: true,
        isLoading: false,
        error: null,
      });

      console.log("[NodeRegistryStore] Initialization complete:", {
        nodeDefinitions: defs.length,
      });
    } catch (error) {
      console.error("[NodeRegistryStore] Failed to sync from backend:", error);
      set({
        isLoading: false,
        error: error instanceof Error ? error.message : String(error),
      });
      throw error;
    }
  },

  // 清空节点定义
  clear: () => {
    set({
      definitions: new Map(),
      definitionsArray: [],
      isInitialized: false,
      error: null,
    });
  },

  // 获取特定类型的节点定义
  getDefinition: (type: string) => {
    return get().definitions.get(type);
  },

  // 获取所有节点定义 - 返回缓存的数组
  getAllDefinitions: () => {
    return get().definitionsArray;
  },

  // 检查是否存在某个类型的节点定义
  hasDefinition: (type: string) => {
    return get().definitions.has(type);
  },
}));

// 选择器 hooks - 直接访问缓存的数组
export const useNodeDefinitions = () => useNodeRegistryStore((s) => s.definitionsArray);
export const useIsNodeRegistryInitialized = () => useNodeRegistryStore((s) => s.isInitialized);
export const useIsNodeRegistryLoading = () => useNodeRegistryStore((s) => s.isLoading);
export const useNodeRegistryError = () => useNodeRegistryStore((s) => s.error);
