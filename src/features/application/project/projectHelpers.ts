/**
 * dataStore 项目相关辅助函数
 * 替代原 @/features/core/project 中的 helpers
 */

import { buildGraphSnapshotFromStores } from '@/features/core/dataStore/projectSnapshotBridge';
import type { GraphSnapshotData } from '@/shared/types/store/graph';

/**
 * 从三 store 组装指定 graph 的完整数据（ResourceStore + graphMetaStore + GraphDataStore）。
 */
export function getGraphByPath(graphPath: string): GraphSnapshotData | null {
  return buildGraphSnapshotFromStores()[graphPath] ?? null;
}
