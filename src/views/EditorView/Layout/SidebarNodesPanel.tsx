import { useMemo } from 'react';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { buildBuiltinCatalogItems, buildNodeTemplateDragData, type NodeCatalogItem } from '@/features/domain/nodeCatalog';
import { focusDetailOnNodeDefinition } from '@/features/core/editor/detail/detailFocusCommands';
import { useDetailTarget } from '@/features/core/editor';
import { NodeCatalogTreeView } from './nodeCatalog/NodeCatalogTreeView';

export function SidebarNodesPanel() {
  const definitions = useNodeRegistryStore((s) => s.definitionsArray);
  const items = useMemo(() => buildBuiltinCatalogItems(definitions), [definitions]);
  const detailTarget = useDetailTarget();

  const selectedNodeType =
    detailTarget?.kind === 'nodeDefinition' ? detailTarget.nodeType : null;

  const handleLeafClick = (item: NodeCatalogItem) => {
    focusDetailOnNodeDefinition(item.nodeType);
  };

  return (
    <NodeCatalogTreeView
      items={items}
      variant="sidebar"
      selectedKey={selectedNodeType}
      onLeafClick={handleLeafClick}
      getLeafDragData={(item) => buildNodeTemplateDragData(item)}
      autoFocusSearch={false}
    />
  );
}
