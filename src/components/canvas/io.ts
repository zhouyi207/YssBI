import { BaseNode } from "../node/models";
import { CanvasState } from "./type";
import { NODE_REGISTRY } from "../node/registry";

/**
 * 节点编辑器数据协议格式 (Schema)
 */
export interface GraphData {
  version: string;
  canvas: CanvasState;
  nodes: {
    id: string;
    type: string;
    title: string;
    position: { x: number; y: number };
    selected?: boolean;
    inputs: any[];
    outputs: any[];
  }[];
  variables?: Record<string, { name: string; type: string; value: any }>;
  metadata: {
    exportTime: string;
    appVersion: string;
  };
}

const CURRENT_VERSION = "1.0.0";

/**
 * 将当前画布状态序列化为可存储的 JSON 对象
 */
export function serializeGraph(
  nodes: BaseNode[],
  canvas: CanvasState,
  variables?: Record<string, { name: string; type: string; value: any }>
): GraphData {
  return {
    version: CURRENT_VERSION,
    canvas,
    nodes: nodes.map((node) => ({
      id: node.id,
      type: node.type,
      title: node.title,
      position: node.position,
      selected: node.selected,
      variableId: node.variableId,
      inputs: node.inputs.map((p) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        links: p.links,
        defaultValue: p.defaultValue,
      })),
      outputs: node.outputs.map((p) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        links: p.links,
        defaultValue: p.defaultValue,
      })),
    })),
    variables,
    metadata: {
      exportTime: new Date().toISOString(),
      appVersion: "0.1.0",
    },
  };
}

/**
 * 将 JSON 对象反序列化为运行时的节点实例
 */
export function deserializeGraph(data: any): { 
  nodes: BaseNode[]; 
  canvas: CanvasState; 
  variables?: Record<string, { name: string; type: string; value: any }> 
} {
  if (!data || data.version !== CURRENT_VERSION) {
    console.warn("Graph data version mismatch or invalid data");
  }

  const nodes = (data.nodes || []).map((n: any) => {
    const def = NODE_REGISTRY[n.type];
    if (!def) {
      console.error(`Unknown node type: ${n.type}`);
      return null;
    }

    // 实例化具体的节点类
    const node = new def.className(
      n.id,
      n.type,
      n.title,
      n.position,
      ...(def.extraArgs || [])
    );

    node.selected = !!n.selected;
    node.variableId = n.variableId;

    // 恢复 Pins 数据，确保 ID 和连接关系完整
    node.inputs = (n.inputs || []).map((p: any) => ({
      ...p,
      nodeId: n.id,
      direction: "input",
    }));

    node.outputs = (n.outputs || []).map((p: any) => ({
      ...p,
      nodeId: n.id,
      direction: "output",
    }));

    return node;
  }).filter(Boolean) as BaseNode[];

  return {
    nodes,
    canvas: data.canvas || { x: 0, y: 0, scale: 1 },
    variables: data.variables,
  };
}
