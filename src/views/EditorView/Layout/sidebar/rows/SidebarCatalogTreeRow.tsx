import {
  VscSymbolMethod,
  VscSymbolProperty,
} from 'react-icons/vsc';
import { DRAG_TYPES, type NodeTemplateDragData } from '@/features/core/dnd';
import { catalogItemKey } from '@/features/domain/nodeCatalog/catalogItem';
import type { LocalizedCatalogBrowserRow } from '@/features/domain/nodeCatalog/localizedCatalogTree';
import { cn } from '@/lib/utils';
import { SidebarDraggableItem, SidebarTreeCategoryRow } from '../../sidebarUi';
import {
  SIDEBAR_ROW_ICON_SIZE,
  sidebarItemIndent,
  sidebarItemRowClass,
  SIDEBAR_ROW_LEADING_SLOT_CLASS,
} from '../../sidebarUi/sidebarStyles';

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
  if (row.kind === 'category') {
    return (
      <SidebarTreeCategoryRow
        categoryId={row.category.categoryId}
        label={row.category.title}
        depth={row.depth}
        expanded={expanded}
        interactionDisabled={interactionDisabled}
        onExpandedChange={onExpandedChange}
      />
    );
  }

  const dragData = {
    type: DRAG_TYPES.NODE_TEMPLATE,
    template: {
      title: row.item.title,
      descriptor: row.item.creation,
    },
  } satisfies NodeTemplateDragData;
  const NodeIcon = row.item.creation.kind === 'resourceBound'
    ? VscSymbolMethod
    : VscSymbolProperty;

  return (
    <SidebarDraggableItem
      id={`node-${catalogItemKey(row.item)}`}
      dragData={dragData}
      className={cn(
        sidebarItemRowClass(false),
        'h-8 min-h-8 rounded-md py-0 transition-none hover:bg-sidebar-accent/50',
      )}
      style={sidebarItemIndent(row.depth)}
    >
      <span className={SIDEBAR_ROW_LEADING_SLOT_CLASS} aria-hidden>
        <NodeIcon
          size={SIDEBAR_ROW_ICON_SIZE}
          className="text-sidebar-foreground/55 transition-colors group-hover:text-sidebar-foreground/80"
        />
      </span>
      <span
        className="min-w-0 flex-1 truncate text-[12px] leading-normal text-sidebar-foreground/80"
        title={row.item.nodeTypeId}
      >
        {row.item.title}
      </span>
    </SidebarDraggableItem>
  );
}
