/**
 * dataStore 项目相关辅助函数
 * 替代原 @/features/core/project 中的 helpers
 */

import { LoadStatus } from '@/shared/types/ui/common';
import { ProjectData } from '@/shared/types';
import { useProjectIOStore } from './projectIOStore';
import { buildGraphSnapshotFromStores } from './projectSnapshotBridge';
import type { GraphData } from '@/shared/types/store/graph';

/**
 * 从三 store 组装指定 graph 的完整数据（ResourceStore + graphMetaStore + GraphDataStore）。
 */
export function getGraphByPath(graphPath: string): GraphData | null {
  return buildGraphSnapshotFromStores()[graphPath] ?? null;
}

/**
 * 初始化时从后端同步项目状态
 * 应该在应用启动时调用一次
 *
 * - 如果 Project 已 Ready，会触发同步
 * - 如果已经 Ready，直接返回当前数据
 */
export async function initProjectSync(): Promise<ProjectData | null> {
  const { status, loadProject, exportSnapshot } = useProjectIOStore.getState();

  if (status === LoadStatus.Ready) {
    return exportSnapshot();
  }

  return await loadProject();
}
