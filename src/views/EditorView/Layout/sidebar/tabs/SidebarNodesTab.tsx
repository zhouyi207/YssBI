import { useTranslation } from 'react-i18next';
import { nodeCatalogErrorText } from '@/features/application/nodeCatalog/nodeCatalogErrorPresentation';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import { DRAG_TYPES, type NodeTemplateDragData } from '@/features/core/dnd';
import { ScrollArea } from '@/components/ui/scroll-area';
import { SidebarDraggableItem } from '../../sidebarUi';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';

export function SidebarNodesTab() {
  const { t } = useTranslation();
  const { status, error, catalog } = useLocalizedNodeCatalog();

  return (
    <SidebarTabPanel>
      <ScrollArea className="min-h-0 flex-1">
        <div className="space-y-1 p-2">
          {status === 'error' && !catalog ? (
            <p role="alert" className="px-2 py-3 text-sm text-destructive">
              {nodeCatalogErrorText(error, t)}
            </p>
          ) : !catalog ? (
            <p role="status" className="px-2 py-3 text-sm text-muted-foreground">
              {t('common.loading')}
            </p>
          ) : catalog.items.map((item) => {
            const itemKey = item.creation.kind === 'resourceBound'
              ? `${item.creation.kind}:${item.nodeTypeId}:${item.creation.resourcePath}`
              : `${item.creation.kind}:${item.nodeTypeId}`;
            const dragData = {
              type: DRAG_TYPES.NODE_TEMPLATE,
              template: {
                title: item.title,
                descriptor: item.creation,
              },
            } satisfies NodeTemplateDragData;
            return (
              <SidebarDraggableItem
                key={itemKey}
                id={`node-${itemKey}`}
                dragData={dragData}
                className="rounded-sm px-2 py-1.5"
              >
                <div className="truncate text-xs text-foreground">{item.title}</div>
                <div className="truncate font-mono text-[10px] text-muted-foreground">
                  {item.nodeTypeId}
                </div>
              </SidebarDraggableItem>
            );
          })}
        </div>
      </ScrollArea>
    </SidebarTabPanel>
  );
}
