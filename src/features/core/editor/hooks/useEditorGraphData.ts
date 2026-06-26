/**
 * 编辑器图数据占位 Hook
 *
 * 节点渲染已改为「逐节点 store 订阅」（见 `useNodeView` / `CanvasNode`），
 * 不再在此对整图执行 `deserializeGraph`——那会在每次 store 变更时于
 * Sidebar / Menubar / Detail 等多个组件里重复触发 O(节点 × pin) 的重建，
 * 是节点创建/连接卡顿的主因。
 *
 * 图变量来自 `variableStore`，这里仅保留空 `variables` 占位以兼容现有消费方。
 * `deserializeGraph` 仍用于保存/序列化等按需场景。
 */

import type { Variable } from '@/shared/types/domain';

const EMPTY_VARIABLES: Record<string, Variable> = {};
const EMPTY_GRAPH_DATA = { variables: EMPTY_VARIABLES } as const;

export function useEditorGraphData() {
  return EMPTY_GRAPH_DATA;
}
