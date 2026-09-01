import { uiStore } from "@/features/core/ui/UIStore";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { deleteResource } from "@/features/application/resource/resourceActions";

function graphDisplayName(path: string, kind: "event" | "function"): string {
  return useResourceStore.getState().resources[resourceKey({ id: path, kind })]?.name ?? path;
}

async function deleteFunctionWithConfirm(functionPath: string): Promise<boolean> {
  const name = graphDisplayName(functionPath, "function");
  const confirmed = await uiStore.confirm({
    title: "删除函数",
    message: `确定要删除函数「${name}」吗？相关引用将由项目事务统一更新。`,
    confirmText: "删除",
    cancelText: "取消",
    type: "danger",
  });
  if (!confirmed) return false;
  await deleteResource({ id: functionPath, kind: "function" });
  return true;
}

/** Unified graph delete with confirm — function adds call-site options; event uses simple confirm. */
export async function deleteGraphWithConfirm(
  graphPath: string,
  kind: "event" | "function",
): Promise<boolean> {
  if (kind === "function") {
    return deleteFunctionWithConfirm(graphPath);
  }

  const name = graphDisplayName(graphPath, "event");
  const confirmed = await uiStore.confirm({
    title: "删除 Event 图",
    message: `确定要删除 Event 图「${name}」吗？`,
    confirmText: "删除",
    cancelText: "取消",
    type: "danger",
  });
  if (!confirmed) return false;
  await deleteResource({ id: graphPath, kind: "event" });
  return true;
}
