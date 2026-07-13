import { memo } from 'react';
import { buildNodeTemplateDragData, type NodeCatalogItem } from '@/features/domain/nodeCatalog';
import { SidebarListItem } from '../../sidebarUi';
import { nodeCatalogItemIcon } from '../../nodeCatalog/nodeCatalogIcons';

export const SidebarNodeRow = memo(function SidebarNodeRow({
  item,
  level,
  selected,
  onClick,
}: {
  item: NodeCatalogItem;
  level: number;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <SidebarListItem
      id={`catalog-${item.nodeType}`}
      dragData={buildNodeTemplateDragData(item)}
      isSelected={selected}
      indentDepth={level}
      icon={nodeCatalogItemIcon(item)}
      label={item.title}
      onClick={onClick}
    />
  );
});
