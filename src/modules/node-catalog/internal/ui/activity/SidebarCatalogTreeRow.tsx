import { DRAG_TYPES } from "@/features/core/dnd";
import type { NodeTemplateDragData } from "@/features/core/dnd";
import { catalogItemKey } from "@/features/domain/nodeCatalog/catalogItem";
import type { LocalizedCatalogBrowserRow } from "@/features/domain/nodeCatalog/localizedCatalogTree";
import { LocalizedCatalogTreeRow } from "./LocalizedCatalogTreeRow";

export function SidebarCatalogTreeRow({
  row,
  expanded,
  interactionDisabled,
  onExpandedChange,
}: {
  row: LocalizedCatalogBrowserRow;
  expanded: boolean;
  interactionDisabled: boolean;
  onExpandedChange: (expanded: boolean) => void;
}) {
  const dragData =
    row.kind === "item"
      ? ({
          type: DRAG_TYPES.NODE_TEMPLATE,
          template: {
            title: row.item.title,
            descriptor: row.item.creation,
          },
        } satisfies NodeTemplateDragData)
      : null;

  return (
    <LocalizedCatalogTreeRow
      row={row}
      expanded={expanded}
      interactionDisabled={interactionDisabled}
      onExpandedChange={onExpandedChange}
      dragData={dragData}
      dragId={row.kind === "item" ? `node-${catalogItemKey(row.item)}` : undefined}
    />
  );
}
