/**
 * dataStore 公共出口
 *
 * 分层：
 * - *Store：Zustand 状态
 * - projectSnapshot / projectSnapshotBridge：图快照纯函数与跨 store 桥接
 * - projectHelpers：initProjectSync / getGraphByPath 等应用辅助
 * - projectIOStore：load / refreshResourceIndex / loadGraph 编排
 *
 * 跨 store 依赖须集中在 projectSnapshotBridge / projectClientReset / projectIOStore。
 */

export * from './databaseStore';
export * from './columnStatsStore';
export * from './columnDistributionStore';
export * from './datasetOverviewStore';
export * from './editStateStore';
export * from './graphMetaStore';
export * from './variableStore';
export * from './graphEntityAccess';
export * from './graphDataStore';

export { useNodeView } from './useNodeView';
export {
  REROUTE_NODE_STYLE_ID,
  toUiNode,
  uiNodeHasNoHeader,
  uiNodeIsReroute,
} from './nodeView';
export type { ToUiNodeOptions, UiNodePinSlice } from './nodeView';
export { findInternalNodeInGraph } from './graphNodeSelectors';

export { buildGraphSnapshot, type GraphSnapshotAccess } from './projectSnapshot';
export { buildGraphSnapshotFromStores } from './projectSnapshotBridge';
export { resetClientProjectState } from './projectClientReset';
