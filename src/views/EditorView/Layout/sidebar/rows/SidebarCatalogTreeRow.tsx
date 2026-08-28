import { DRAG_TYPES, catalogItemKey, type NodeTemplateDragData, type LocalizedCatalogBrowserRow } from '@/features/application/viewCapabilities';
import { LocalizedCatalogTreeRow } from '../../sidebarUi';

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
  const dragData = row.kind === 'item'
    ? {
        type: DRAG_TYPES.NODE_TEMPLATE,
        template: {
          title: row.item.title,
          descriptor: row.item.creation,
        },
      } satisfies NodeTemplateDragData
    : null;

  return (
    <LocalizedCatalogTreeRow
      row={row}
      expanded={expanded}
      interactionDisabled={interactionDisabled}
      onExpandedChange={onExpandedChange}
      dragData={dragData}
      dragId={row.kind === 'item' ? `node-${catalogItemKey(row.item)}` : undefined}
    />
  );
}
