/**
 * Store `NodeData` + pin 切片 → 画布 `UINode` 单点桥接
 */

import type { NodeData, PinData, PinView } from '@/shared/types/store/graph';
import type { UINode } from '@/shared/types/ui';
import { derivePinConnectionView } from './pinLinks';

export const REROUTE_NODE_STYLE_ID = 'builtin.reroute';

export interface UiNodePinSlice {
  pin: PinData;
  connectionIds: string[];
}

export interface ToUiNodeOptions {
  pins: UiNodePinSlice[];
}

export function uiNodeIsReroute(node: Pick<UINode, 'uiStyle'>): boolean {
  return node.uiStyle === REROUTE_NODE_STYLE_ID;
}

/** math and compact reroute layouts have no independent header area. */
export function uiNodeHasNoHeader(node: Pick<UINode, 'uiStyle'>): boolean {
  return node.uiStyle === 'math' || uiNodeIsReroute(node);
}

/** 由 store 节点数据与 graph-scoped pin 切片构建画布节点视图 */
export function toUiNode(nodeData: NodeData, options: ToUiNodeOptions): UINode {
  const title = nodeData.display?.title ?? nodeData.title;
  const inputs: PinView[] = [];
  const outputs: PinView[] = [];

  for (const { pin, connectionIds } of options.pins) {
    const connectionView = derivePinConnectionView(connectionIds);
    const pinView: PinView = { ...pin, ...connectionView };
    if (pin.direction === 'output') outputs.push(pinView);
    else inputs.push(pinView);
  }

  return {
    id: nodeData.id,
    nodeType: nodeData.nodeType,
    category: nodeData.category,
    title,
    uiStyle: nodeData.display?.styleId ?? 'default',
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
