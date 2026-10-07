import i18n from "i18next";
import {
  createFileResource,
  deleteResource,
  duplicateFileResource,
  fileResourceHandlers,
  type FileResourceRef,
} from "./resourceActions";
import { openFileInEditor } from "@/features/application/editor/openFileInEditor";
import {
  captureProjectIdentity,
  assertCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { uiStore } from "@/features/core/ui/UIStore";
import type { FileResourceKind } from "@/shared/types/domain/resource";

export async function createFile(kind: FileResourceKind, name?: string): Promise<void> {
  const identity = captureProjectIdentity();
  const path = await createFileResource(kind, name);
  assertCurrentProjectIdentity(identity);
  await openFileInEditor(path, kind);
  assertCurrentProjectIdentity(identity);
  useSidebarStore
    .getState()
    .setCategoryExpanded("project", fileResourceHandlers[kind].categoryId, true);
}
export async function duplicateFile(ref: FileResourceRef): Promise<void> {
  const identity = captureProjectIdentity();
  const path = await duplicateFileResource(ref);
  assertCurrentProjectIdentity(identity);
  await openFileInEditor(path, ref.kind);
}
export async function deleteFileWithConfirm(ref: FileResourceRef): Promise<boolean> {
  const identity = captureProjectIdentity();
  const name = useResourceStore.getState().resources[resourceKey(ref)]?.name ?? ref.id;
  const confirmed = await uiStore.confirm({
    title: i18n.t("documents.delete"),
    message: i18n.t("documents.deleteMessage", { name }),
    confirmText: i18n.t("contextMenu.sidebar.delete"),
    cancelText: i18n.t("common.cancel"),
    type: "danger",
  });
  if (!confirmed) return false;
  assertCurrentProjectIdentity(identity);
  await deleteResource(ref);
  return true;
}
