import { invoke } from "@tauri-apps/api/core";
import { NodeDefinition, BaseNode } from "../Types/nodes";
import { Position } from "../../../types";
import { useSchemaStore } from "../Store/useSchemaStore";

export class NodeRegistry {
  private static instance: NodeRegistry;
  private definitions: Map<string, NodeDefinition> = new Map();
  private _isInitialized: boolean = false;

  private constructor() { }

  static getInstance(): NodeRegistry {
    if (!NodeRegistry.instance) {
      NodeRegistry.instance = new NodeRegistry();
    }
    return NodeRegistry.instance;
  }

  get isInitialized(): boolean {
    return this._isInitialized;
  }

  /**
   * 从后端同步节点定义和 Schema
   */
  async syncFromBackend() {
    try {
      // 并行加载节点定义和 schema
      const [defs] = await Promise.all([
        invoke<NodeDefinition[]>("get_node_definitions"),
        useSchemaStore.getState().loadSchema(),
      ]);

      this.definitions.clear();
      defs.forEach(def => {
        this.definitions.set(def.node_type, def);
      });
      
      this._isInitialized = true;
      console.log("[NodeRegistry] Initialization complete:", {
        nodeDefinitions: defs.length,
        schemaLoaded: useSchemaStore.getState().isLoaded,
      });
    } catch (error) {
      console.error("[NodeRegistry] Failed to sync from backend:", error);
      throw error;
    }
  }

  getDefinition(type: string): NodeDefinition | undefined {
    return this.definitions.get(type);
  }

  getAllDefinitions(): NodeDefinition[] {
    return Array.from(this.definitions.values());
  }

  /**
   * 根据类型创建一个新的节点实例
   */
  createNode(type: string, id: string, position: Position): BaseNode | null {
    const def = this.getDefinition(type);
    if (!def) {
      console.error(`Node type ${type} not found in registry`);
      return null;
    }
    return new BaseNode(id, def, position);
  }
}

// 导出单例方便使用
export const NODE_REGISTRY = NodeRegistry.getInstance();
