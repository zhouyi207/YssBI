import { memo } from 'react';
import { buildNodeTemplateDragData, type NodeCatalogItem } from '@/features/domain/nodeCatalog';
import { SidebarListItem } from '../../sidebarUi';
import { nodeCatalogItemIcon } from '../../nodeCatalog/nodeCatalogIcons';
import { EDITOR_MUTATION_CAPABILITIES } from '@/features/application/editor/editorMutationAvailability';

export const SidebarNodeRow = memo(function SidebarNodeRow({
  item,
  level,
  selected,
  onClick,
  creationEnabled = EDITOR_MUTATION_CAPABILITIES.contextualCompatibility,
}: {
  item: NodeCatalogItem;
  level: number;
  selected: boolean;
  onClick: () => void;
  creationEnabled?: boolean;
}) {
  return (
    <SidebarListItem
      id={`catalog-${item.nodeType}`}
      dragData={creationEnabled ? buildNodeTemplateDragData(item) : null}
      isSelected={selected}
      indentDepth={level}
      icon={nodeCatalogItemIcon(item)}
      label={item.title}
      onClick={onClick}
    />
  );
});
