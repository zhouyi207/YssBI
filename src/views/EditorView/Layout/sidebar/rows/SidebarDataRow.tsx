import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { VscDatabase } from 'react-icons/vsc';
import {
  buildSidebarDragData,
  refreshMissingSidebarResourcePath,
} from '@/features/application/sidebar';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import { findResourceNodeSpawnTemplate } from '@/features/application/editor/canvasDrop';
import { openDatabaseEditorWindow } from '@/features/application/window';
import { revealDetails } from '@/features/application/editor/rightSidebarActions';
import { TYPE_ICON_COLORS } from '@/features/application/viewCapabilities';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { SidebarListItem, SidebarRowActionButton, SIDEBAR_ROW_ICON_SIZE } from '../../sidebarUi';

export const SidebarDataRow = memo(function SidebarDataRow({
  id,
  resourcePath,
  name,
  data,
  indentDepth = 0,
  isSelected = false,
  onContextMenu,
}: {
  id: string;
  resourcePath?: string;
  name: string;
  data: unknown;
  indentDepth?: number;
  isSelected?: boolean;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();
  const isLoading = (data as { loading?: unknown }).loading === true;
  const loadFailed = (data as { loadFailed?: unknown }).loadFailed === true;
  const { status, catalog, refresh } = useLocalizedNodeCatalog();
  const templateForPath = (path: string) => status === 'ready' && catalog
    ? findResourceNodeSpawnTemplate(
        catalog.items,
        path,
        'database',
        'yssbi.dataframe.source.get',
      )
    : null;
  const template = resourcePath ? templateForPath(resourcePath) : null;
  const dragData = template
    ? buildSidebarDragData(id, name, 'data', template.descriptor)
    : null;
  const resourceCatalogRefreshMessage = t('notifications.editor.resourceCatalogRefreshing');
  const handleDisabledDragAttempt = () => {
    if (resourcePath) {
      refresh();
    } else {
      void refreshMissingSidebarResourcePath({
        kind: 'database',
        id,
        hasCurrentDescriptor: (path) => templateForPath(path) != null,
        refreshCatalog: refresh,
      });
    }
  };

  return (
    <SidebarListItem
      id={id}
      dragData={dragData}
      dragDisabledReason={resourceCatalogRefreshMessage}
      onDisabledDragAttempt={handleDisabledDragAttempt}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={<VscDatabase size={SIDEBAR_ROW_ICON_SIZE} style={{ color: TYPE_ICON_COLORS.data }} />}
      label={name}
      onClick={async (e) => {
        e.stopPropagation();
        await revealDetails({ kind: 'data', id });
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        void openDatabaseEditorWindow(id);
      }}
      onContextMenu={onContextMenu}
      trailing={
        <>
          {isLoading && (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-amber-400 animate-pulse" />
              </TooltipTrigger>
              <TooltipContent side="top">{t('sidebar.dataLoading')}</TooltipContent>
            </Tooltip>
          )}
          {!isLoading && loadFailed && (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-red-500" />
              </TooltipTrigger>
              <TooltipContent side="top">{t('sidebar.dataLoadFailed')}</TooltipContent>
            </Tooltip>
          )}
          <SidebarRowActionButton
            isSelected={isSelected}
            tooltip={t('sidebar.viewInDatabaseEditor')}
            onClick={(e) => {
              e.stopPropagation();
              void openDatabaseEditorWindow(id);
            }}
          />
        </>
      }
    />
  );
});
