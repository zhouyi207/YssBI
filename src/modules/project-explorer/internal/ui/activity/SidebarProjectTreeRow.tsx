import { memo } from "react";
import { isActivityFileItem, type ActivityItem } from "@/shared/types/domain/activityPanel";
import type { ProjectTreeCategoryId } from "@/features/core/sidebar/projectTreeState";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import type { FileResourceRef } from "@/features/application/resource/resourceActions";
import { SidebarFileRow } from "./SidebarFileRow";
import { SidebarDataRow } from "./SidebarDataRow";

export interface SidebarProjectTreeActions {
  onCreateFile(kind: FileResourceKind): void;
  onOpenFile(ref: FileResourceRef): void;
  onFileContextMenu(event: React.MouseEvent, ref: FileResourceRef & { name: string }): void;
  onImportData(): void;
  onCategoryContextMenu(event: React.MouseEvent, categoryId: ProjectTreeCategoryId): void;
  onDatabaseContextMenu(event: React.MouseEvent, id: string, name: string): void;
}
export const SidebarProjectTreeRow = memo(function SidebarProjectTreeRow({
  item,
  depth,
  actions,
  isSelected,
}: {
  item: ActivityItem;
  depth: number;
  actions: SidebarProjectTreeActions;
  isSelected: boolean;
}) {
  if (isActivityFileItem(item))
    return (
      <SidebarFileRow
        id={item.path}
        name={item.name}
        kind={item.kind}
        indentDepth={depth}
        isSelected={isSelected}
        onOpen={actions.onOpenFile}
        onContextMenu={(event) =>
          actions.onFileContextMenu(event, { id: item.path, kind: item.kind, name: item.name })
        }
      />
    );
  if (item.kind === "database")
    return (
      <SidebarDataRow
        id={item.id}
        name={item.name}
        resourcePath={item.resourcePath}
        indentDepth={depth}
        isSelected={isSelected}
        onContextMenu={(event) => actions.onDatabaseContextMenu(event, item.id, item.name)}
      />
    );
  return null;
});
