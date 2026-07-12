/**
 * dataStore 公共出口
 *
 * 分层：
 * - *Store：Zustand 状态
 * - projectSnapshot / projectSnapshotBridge：图导出纯函数与跨 store 桥接
 * - projectHelpers：initProjectSync / getGraphByPath 等应用辅助
 * - projectIOStore：load / export / loadGraph 编排
 *
 * 跨 store 依赖须集中在 projectSnapshotBridge / projectClientReset / projectIOStore，
 * 并由 projectStoreDeps.audit 单测校验显式 import。
 */

export * from './databaseStore';
export * from './columnStatsStore';
export * from './columnDistributionStore';
export * from './datasetOverviewStore';
export * from './editStateStore';
export * from './graphMetaStore';
export * from './graphRuntimeStore';
export * from './projectIOStore';
export * from './projectHelpers';
export * from './variableStore';
export * from './graphEntityAccess';
export * from './graphDataStore';
export { resolveNodeViewMeta } from '@/features/domain/nodeViewMeta';
export { useNodeView } from './useNodeView';
export { toUiNode, uiNodeHasNoHeader } from './nodeView';
export type { ToUiNodeOptions, UiNodePinSlice } from './nodeView';
export { findInternalNodeInGraph } from './graphNodeSelectors';

export { buildGraphSnapshot, type GraphSnapshotAccess } from './projectSnapshot';
export { buildGraphSnapshotFromStores } from './projectSnapshotBridge';
export { resetClientProjectState } from './projectClientReset';
