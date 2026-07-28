import { uiStore } from '@/features/core/ui/UIStore';
import { resourceKey, useResourceStore } from '@/features/core/resource';
import { GraphService } from '@/services/graph/graphService';
import { deleteResource } from '@/features/application/resource/resourceActions';
import { invalidateGraphProjections } from '@/features/application/editorProjection/graphProjectionCoordinator';

function graphDisplayName(path: string, kind: 'event' | 'function'): string {
  return useResourceStore.getState().resources[resourceKey({ id: path, kind })]?.name ?? path;
}

function countCallSites(sites: { nodeIds: string[] }[]): number {
  return sites.reduce((sum, site) => sum + site.nodeIds.length, 0);
}

async function deleteFunctionWithCallSiteConfirm(functionPath: string): Promise<boolean> {
  const name = graphDisplayName(functionPath, 'function');
  const callSites = await GraphService.getFunctionCallSites(functionPath);
  const callCount = countCallSites(callSites);

  if (callCount === 0) {
    const confirmed = await uiStore.confirm({
      title: '删除函数',
      message: `确定要删除函数「${name}」吗？`,
      confirmText: '删除',
      cancelText: '取消',
      type: 'danger',
    });
    if (!confirmed) return false;
    await deleteResource({ id: functionPath, kind: 'function' });
    return true;
  }

  const result = await uiStore.confirm3({
    title: '删除函数',
    message: `函数「${name}」被 ${callCount} 处 Call 节点引用。仍删除将保留这些 Call 节点（目标失效）；「删除并清理」将一并移除所有 Call 节点。`,
    confirmText: '仍删除',
    discardText: '删除并清理 Call',
    cancelText: '取消',
    type: 'danger',
  });

  if (result === 'cancel') return false;

  if (result === 'discard') {
    const callerGraphs = await GraphService.purgeFunctionCallSites(functionPath);
    await invalidateGraphProjections(callerGraphs.map((graph) => graph.path));
  }

  await deleteResource({ id: functionPath, kind: 'function' });
  return true;
}

/** Unified graph delete with confirm — function adds call-site options; event uses simple confirm. */
export async function deleteGraphWithConfirm(
  graphPath: string,
  kind: 'event' | 'function',
): Promise<boolean> {
  if (kind === 'function') {
    return deleteFunctionWithCallSiteConfirm(graphPath);
  }

  const name = graphDisplayName(graphPath, 'event');
  const confirmed = await uiStore.confirm({
    title: '删除 Event 图',
    message: `确定要删除 Event 图「${name}」吗？`,
    confirmText: '删除',
    cancelText: '取消',
    type: 'danger',
  });
  if (!confirmed) return false;
  await deleteResource({ id: graphPath, kind: 'event' });
  return true;
}
