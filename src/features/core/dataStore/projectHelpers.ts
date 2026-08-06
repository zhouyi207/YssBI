/**
 * dataStore 项目相关辅助函数
 * 替代原 @/features/core/project 中的 helpers
 */

import { LoadStatus } from '@/shared/types/ui/common';
import { ProjectData } from '@/shared/types';
import { loadActivatedProject, useProjectIOStore } from './projectIOStore';
import { reconcileProjectPath } from './projectSession';
import { captureProjectLifecycleState } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { ProjectService } from '@/services/project/projectService';
import { buildGraphSnapshotFromStores } from './projectSnapshotBridge';
import type { GraphData } from '@/shared/types/store/graph';

/**
 * 从三 store 组装指定 graph 的完整数据（ResourceStore + graphMetaStore + GraphDataStore）。
 */
export function getGraphByPath(graphPath: string): GraphData | null {
  return buildGraphSnapshotFromStores()[graphPath] ?? null;
}

/**
 * 初始化时从后端同步项目状态（可重复调用；与 `loadProject` 合并并发）。
 *
 * - Ready + 有 `currentPath`：轻量返回快照
 * - Ready + 无 path 但后端有会话：全量 `loadProject` 重灌前端投影
 * - 其它：全量 `loadProject`
 */
export async function initProjectSync(): Promise<ProjectData | null> {
  if (!captureProjectLifecycleState().projectInstanceId) {
    return loadActivatedProject(await ProjectService.getProjectActivation());
  }

  const { status, currentPath, loadProject, exportSnapshot } = useProjectIOStore.getState();

  if (status === LoadStatus.Ready) {
    if (currentPath) {
      return exportSnapshot();
    }
    const reconciled = await reconcileProjectPath();
    if (!reconciled) {
      return exportSnapshot();
    }
    return await loadProject();
  }

  return await loadProject();
}
