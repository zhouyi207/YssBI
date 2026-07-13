import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { buildBuiltinCatalogItems } from '@/features/domain/nodeCatalog';
import { buildNodesFlatRows, useSidebarStore } from '@/features/core/sidebar';
import { focusDetailOnNodeDefinition } from '@/features/core/editor/detail/detailFocusCommands';
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog';
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue';
import { NodeCatalogSearchBar } from '../../nodeCatalog/NodeCatalogSearchBar';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';
import { SidebarFlatRowPanel } from '../sections/SidebarFlatRowPanel';

export function SidebarNodesTab() {
  const { t } = useTranslation();
  const definitions = useNodeRegistryStore((s) => s.definitionsArray);
  const expandedGroups = useSidebarStore((s) => s.expandedGroups);
  const toggleGroup = useSidebarStore((s) => s.toggleGroup);
  const toggleSection = useSidebarStore((s) => s.toggleSection);

  const [queryRaw, setQueryRaw] = useState('');
  const filterQuery = useDebouncedValue(queryRaw, 150);

  const items = useMemo(() => buildBuiltinCatalogItems(definitions), [definitions]);

  const rows = useMemo(
    () =>
      buildNodesFlatRows({
        items,
        filterQuery,
        expandedGroups,
        noMatchesMessage: t('canvas.nodePalette.noMatches'),
      }),
    [expandedGroups, filterQuery, items, t],
  );

  const handleLeafClick = (item: NodeCatalogItem) => {
    focusDetailOnNodeDefinition(item.nodeType);
  };

  const emptyCatalog = items.length === 0 && !filterQuery.trim();

  return (
    <SidebarTabPanel footer={<NodeCatalogSearchBar value={queryRaw} onChange={setQueryRaw} />}>
      <SidebarFlatRowPanel
        rows={
          emptyCatalog
            ? [
                {
                  kind: 'empty',
                  rowKey: 'empty:nodes-catalog',
                  level: 0,
                  message: t('sidebar.noBuiltinNodes'),
                },
              ]
            : rows
        }
        onToggleSection={toggleSection}
        onToggleGroup={toggleGroup}
        onNodeLeafClick={handleLeafClick}
      />
    </SidebarTabPanel>
  );
}
