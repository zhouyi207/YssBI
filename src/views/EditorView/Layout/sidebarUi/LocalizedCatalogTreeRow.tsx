import { VscSymbolMethod, VscSymbolProperty } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import type { SidebarDragPayload } from '@/features/core/dnd';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { LocalizedCatalogBrowserRow } from '@/features/domain/nodeCatalog/localizedCatalogTree';
import { cn } from '@/lib/utils';
import { SidebarDraggableItem } from './SidebarDraggableItem';
import { SidebarTreeCategoryRow } from './SidebarTreeCategoryRow';
import {
  SIDEBAR_ROW_ICON_SIZE,
  SIDEBAR_ROW_LEADING_SLOT_CLASS,
  sidebarItemRowClass,
} from './sidebarStyles';

export interface LocalizedCatalogTreeRowProps {
  row: LocalizedCatalogBrowserRow;
  expanded: boolean;
  interactionDisabled?: boolean;
  onExpandedChange: (expanded: boolean) => void;
  onItemSelect?: (descriptor: NodeCreationDescriptor) => void;
  dragData?: SidebarDragPayload | null;
  dragId?: string;
}

const catalogItemRowClass = cn(
  sidebarItemRowClass(false),
  'h-8 min-h-8 justify-start gap-2 px-2 py-0 text-left font-normal transition-none hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
);

function CatalogItemContent({
  row,
}: {
  row: Extract<LocalizedCatalogBrowserRow, { kind: 'item' }>;
}) {
  const NodeIcon = row.item.creation.kind === 'resourceBound'
    ? VscSymbolMethod
    : VscSymbolProperty;

  return (
    <>
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
    </>
  );
}

export function LocalizedCatalogTreeRow({
  row,
  expanded,
  interactionDisabled = false,
  onExpandedChange,
  onItemSelect,
  dragData = null,
  dragId,
}: LocalizedCatalogTreeRowProps) {
  if (row.kind === 'category') {
    return (
      <SidebarTreeCategoryRow
        categoryId={row.category.categoryId}
        label={row.category.title}
        depth={row.depth}
        expanded={expanded}
        interactionDisabled={interactionDisabled}
        onExpandedChange={onExpandedChange}
        dataAttributes={{
          'data-catalog-category-id': row.category.categoryId,
          'data-catalog-depth': row.depth,
        }}
      />
    );
  }

  const content = <CatalogItemContent row={row} />;
  const handleSelect = onItemSelect
    ? () => onItemSelect(row.item.creation)
    : undefined;

  if (dragData) {
    return (
      <SidebarDraggableItem
        id={dragId ?? row.rowKey}
        dragData={dragData}
        className={catalogItemRowClass}
        style={{ paddingLeft: 16 + row.depth * 16 }}
      >
        {content}
      </SidebarDraggableItem>
    );
  }

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      data-catalog-item-key={row.rowKey}
      className={catalogItemRowClass}
      style={{ paddingLeft: 16 + row.depth * 16, paddingRight: 8 }}
      onClick={handleSelect}
    >
      {content}
    </Button>
  );
}
