/**
 * Store `NodeData` + pin 切片 → 画布 `UINode` 单点桥接
 */

import type {
  NodeData,
  PinData,
  PinView,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import type {
  DiagnosticDto,
  NodeDisplayDto,
  ParameterEditorDto,
} from "@/shared/types/domain/editorProjection";
import { derivePinConnectionView } from "./pinLinks";

export interface UINode {
  id: string;
  nodeType: string;
  title: string;
  styleId: string;
  position: { x: number; y: number };
  display: NodeDisplayDto;
  parameterEditors: ParameterEditorDto[];
  diagnostics: DiagnosticDto[];
  inputs: PinView[];
  outputs: PinView[];
}

export const REROUTE_NODE_STYLE_ID = "builtin.reroute";

export interface UiNodePinSlice {
  pin: PinData;
  connectionIds: string[];
}

export interface ToUiNodeOptions {
  pins: UiNodePinSlice[];
}

export function isRerouteNodeView(node: Pick<UINode, "styleId">): boolean {
  return node.styleId === REROUTE_NODE_STYLE_ID;
}

/** 由 store 节点数据与 graph-scoped pin 切片构建画布节点视图 */
export function toUiNode(nodeData: NodeData, options: ToUiNodeOptions): UINode {
  const inputs: PinView[] = [];
  const outputs: PinView[] = [];

  for (const { pin, connectionIds } of options.pins) {
    const connectionView = derivePinConnectionView(connectionIds);
    const pinView: PinView = { ...pin, ...connectionView };
    if (pin.direction === "output") outputs.push(pinView);
    else inputs.push(pinView);
  }

  return {
    id: nodeData.id,
    nodeType: nodeData.nodeType,
    title: nodeData.display.title,
    styleId: nodeData.display.styleId ?? "builtin.default",
    display: nodeData.display,
    parameterEditors: nodeData.parameterEditors,
    diagnostics: nodeData.diagnostics,
    position: nodeData.position,
    inputs,
    outputs,
  };
}
