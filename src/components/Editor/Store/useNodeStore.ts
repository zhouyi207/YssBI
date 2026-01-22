import { create } from 'zustand';
import { BaseNode } from '../Types/nodes';

interface NodeStore {
  nodes: Record<string, BaseNode>;
  // 当前活动 Tab 的所有节点 ID 列表
  activeNodeIds: string[];
  
  // 动作
  setNodes: (nodes: BaseNode[]) => void;
  updateNode: (id: string, updater: (prev: BaseNode) => BaseNode) => void;
  updateNodePosition: (id: string, dx: number, dy: number) => void;
}

export const useNodeStore = create<NodeStore>((set) => ({
  nodes: {},
  activeNodeIds: [],

  setNodes: (nodesArray) => {
    const nodesMap: Record<string, BaseNode> = {};
    const ids: string[] = [];
    nodesArray.forEach(n => {
      nodesMap[n.id] = n;
      ids.push(n.id);
    });
    set({ nodes: nodesMap, activeNodeIds: ids });
  },

  updateNode: (id, updater) => set((state) => {
    const node = state.nodes[id];
    if (!node) return state;
    return {
      nodes: {
        ...state.nodes,
        [id]: updater(node)
      }
    };
  }),

  updateNodePosition: (id, dx, dy) => set((state) => {
    const node = state.nodes[id];
    if (!node) return state;
    
    // 关键优化：直接修改位置对象以获取最高性能，或者返回新对象触发订阅
    const newNode = node.clone();
    newNode.position = { x: node.position.x + dx, y: node.position.y + dy };
    
    return {
      nodes: {
        ...state.nodes,
        [id]: newNode
      }
    };
  }),
}));
