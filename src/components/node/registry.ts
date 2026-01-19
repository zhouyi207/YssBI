import { invoke } from "@tauri-apps/api/core";
import { NodeDefinition, BaseNode } from "./models";
import { Position } from "../../types";

export class NodeRegistry {
  private static instance: NodeRegistry;
  private definitions: Map<string, NodeDefinition> = new Map();

  private constructor() {}

  static getInstance(): NodeRegistry {
    if (!NodeRegistry.instance) {
      NodeRegistry.instance = new NodeRegistry();
    }
    return NodeRegistry.instance;
  }

  /**
   * 从后端同步节点定义
   */
  async syncFromBackend() {
    try {
      const defs = await invoke<NodeDefinition[]>("get_node_definitions");
      this.definitions.clear();
      defs.forEach(def => {
        this.definitions.set(def.node_type, def);
      });
      console.log("Node definitions synced:", defs);
    } catch (error) {
      console.error("Failed to sync node definitions:", error);
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
