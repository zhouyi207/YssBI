/**
 * dataStore 项目相关辅助函数
 * 替代原 @/features/core/project 中的 helpers
 */

import { LoadStatus } from '@/shared/types/ui/common';
import { loadActivatedProject, useProjectIOStore } from './projectIOStore';
import { reconcileProjectPath } from './projectSession';
import { captureProjectLifecycleState } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { ProjectService } from '@/services/project/projectService';
import { buildGraphSnapshotFromStores } from './projectSnapshotBridge';
import type { GraphSnapshotData } from '@/shared/types/store/graph';

/**
 * 从三 store 组装指定 graph 的完整数据（ResourceStore + graphMetaStore + GraphDataStore）。
 */
export function getGraphByPath(graphPath: string): GraphSnapshotData | null {
  return buildGraphSnapshotFromStores()[graphPath] ?? null;
}

/**
 * 显式 hydrate 前端项目投影（可重复调用；与 `loadProject` 合并并发）。
 *
 * - 无前端项目 identity：从后端 activation hydrate
 * - Ready + 有 `currentPath`：投影已就绪，无操作
 * - Ready + 无 path 但后端有会话：全量 `loadProject` 重灌前端投影
 * - 其它：全量 `loadProject`
 */
export async function initProjectSync(): Promise<void> {
  if (!captureProjectLifecycleState().projectInstanceId) {
    await loadActivatedProject(await ProjectService.getProjectActivation());
    return;
  }

  const { status, currentPath, loadProject } = useProjectIOStore.getState();

  if (status === LoadStatus.Ready) {
    if (currentPath) return;

    const reconciled = await reconcileProjectPath();
    if (!reconciled) return;
  }

  await loadProject();
}
