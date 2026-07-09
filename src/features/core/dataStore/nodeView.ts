/**
 * Store `NodeData` + pin 切片 → 画布 `UINode` 单点桥接
 */

import type { NodeData, PinData, PinView } from '@/shared/types/store/graph';
import type { UINode } from '@/shared/types/ui';
import { derivePinConnectionView } from './pinLinks';
import { resolveNodeViewMeta } from '@/features/domain/nodeViewMeta';

export interface UiNodePinSlice {
  pin: PinData;
  connectionIds: string[];
}

export interface ToUiNodeOptions {
  /** 覆盖展示标题（如 Call Function 解析函数名） */
  title?: string;
  pins: UiNodePinSlice[];
}

/** math 布局无独立 header 区域 */
export function uiNodeHasNoHeader(node: Pick<UINode, 'uiStyle'>): boolean {
  return node.uiStyle === 'math';
}

/** 由 store 节点数据与 graph-scoped pin 切片构建画布节点视图 */
export function toUiNode(nodeData: NodeData, options: ToUiNodeOptions): UINode {
  const meta = resolveNodeViewMeta(nodeData);
  const title = options.title ?? meta.title;
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
    nodeType: meta.nodeType,
    category: meta.category,
    title,
    uiStyle: meta.uiStyle,
    description: meta.description,
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
