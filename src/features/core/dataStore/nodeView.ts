/**
 * Store `NodeData` + pin 切片 → 画布 `UINode` 单点桥接
 */

import type {
  NodeData,
  PinData,
  PinView,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { Node as DomainNode } from "@/shared/types/domain/node";
import type {
  DiagnosticDto,
  NodeDisplayDto,
  ParameterEditorDto,
} from "@/shared/types/domain/editorProjection";
import { derivePinConnectionView } from "./pinLinks";

export interface UINode extends Omit<DomainNode, "inputs" | "outputs"> {
  uiStyle: string;
  position: { x: number; y: number };
  isInternal?: boolean;
  paramsKind?: "none" | "variable" | "subGraph" | "dataFrame";
  variableId?: string;
  variableName?: string;
  variableType?: string;
  subGraphPath?: string;
  dataframeId?: string;
  display?: NodeDisplayDto;
  parameterEditors?: ParameterEditorDto[];
  diagnostics?: DiagnosticDto[];
  centerSymbol?: string;
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

export function uiNodeIsReroute(node: Pick<UINode, "uiStyle">): boolean {
  return node.uiStyle === REROUTE_NODE_STYLE_ID;
}

/** math and compact reroute layouts have no independent header area. */
export function uiNodeHasNoHeader(node: Pick<UINode, "uiStyle">): boolean {
  return node.uiStyle === "math" || uiNodeIsReroute(node);
}

/** 由 store 节点数据与 graph-scoped pin 切片构建画布节点视图 */
export function toUiNode(nodeData: NodeData, options: ToUiNodeOptions): UINode {
  const title = nodeData.display?.title ?? nodeData.title;
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
    category: nodeData.category,
    title,
    uiStyle: nodeData.display?.styleId ?? "default",
    display: nodeData.display,
    parameterEditors: nodeData.parameterEditors ?? [],
    diagnostics: nodeData.diagnostics ?? [],
    position: nodeData.position ?? { x: 0, y: 0 },
    isInternal: nodeData.isInternal,
    paramsKind: nodeData.paramsKind,
    variableId: nodeData.variableId,
    variableName: nodeData.variableName,
    variableType: nodeData.variableType,
    subGraphPath: nodeData.subGraphPath,
    dataframeId: nodeData.dataframeId,
    inputs,
    outputs,
  };
}
