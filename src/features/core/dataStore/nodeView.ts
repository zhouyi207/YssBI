/**
 * Store `NodeData` + pin 切片 → 画布 `UINode` 单点桥接
 */

import type { NodeData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";

export interface UINode extends Pick<
  NodeData,
  "id" | "display" | "parameterGroups" | "diagnostics" | "capabilities"
> {
  inputs: PinData[];
  outputs: PinData[];
}

export const REROUTE_NODE_STYLE_ID = "builtin.reroute";

export function isRerouteNodeView(node: Pick<UINode, "display">): boolean {
  return node.display.styleId === REROUTE_NODE_STYLE_ID;
}

/** 由 store 节点数据与 graph-scoped pin 切片构建画布节点视图 */
export function toUiNode(nodeData: NodeData, pins: readonly PinData[]): UINode {
  const inputs: PinData[] = [];
  const outputs: PinData[] = [];

  for (const pin of pins) {
    if (pin.direction === "output") outputs.push(pin);
    else inputs.push(pin);
  }

  return {
    id: nodeData.id,
    display: nodeData.display,
    parameterGroups: nodeData.parameterGroups,
    diagnostics: nodeData.diagnostics,
    capabilities: nodeData.capabilities,
    inputs,
    outputs,
  };
}
