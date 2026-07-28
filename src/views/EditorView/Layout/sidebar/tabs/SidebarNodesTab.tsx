import { NODE_CATALOG_UNAVAILABLE_MESSAGE } from '@/features/application/editor/editorMutationAvailability';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';
import { SidebarFlatRowPanel } from '../sections/SidebarFlatRowPanel';

export function SidebarNodesTab() {
  return (
    <SidebarTabPanel>
      <SidebarFlatRowPanel
        rows={[
          {
            kind: 'empty',
            rowKey: 'empty:nodes-catalog-unavailable',
            level: 0,
            message: NODE_CATALOG_UNAVAILABLE_MESSAGE,
          },
        ]}
        onToggleSection={() => {}}
        onToggleGroup={() => {}}
      />
    </SidebarTabPanel>
  );
}
