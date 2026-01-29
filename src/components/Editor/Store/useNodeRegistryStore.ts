import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { NodeDefinition } from "../Types/nodes";

interface NodeRegistryStore {
  // 状态
  definitions: Map<string, NodeDefinition>;
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
  isInitialized: false,
  isLoading: false,
  error: null,

  // 从后端同步节点定义
  syncFromBackend: async () => {
    set({ isLoading: true, error: null });

    try {
      const defs = await invoke<NodeDefinition[]>("get_node_definitions");

      const definitions = new Map<string, NodeDefinition>();
      defs.forEach(def => {
        definitions.set(def.node_type, def);
      });

      set({
        definitions,
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
      isInitialized: false,
      error: null,
    });
  },

  // 获取特定类型的节点定义
  getDefinition: (type: string) => {
    return get().definitions.get(type);
  },

  // 获取所有节点定义
  getAllDefinitions: () => {
    return Array.from(get().definitions.values());
  },

  // 检查是否存在某个类型的节点定义
  hasDefinition: (type: string) => {
    return get().definitions.has(type);
  },
}));

// 选择器 hooks
export const useNodeDefinitions = () => useNodeRegistryStore((s) => s.getAllDefinitions());
export const useIsNodeRegistryInitialized = () => useNodeRegistryStore((s) => s.isInitialized);
export const useIsNodeRegistryLoading = () => useNodeRegistryStore((s) => s.isLoading);
export const useNodeRegistryError = () => useNodeRegistryStore((s) => s.error);
