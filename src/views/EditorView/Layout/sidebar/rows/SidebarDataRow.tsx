import { logger } from "@/utils/appLogger";
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { VscDatabase } from 'react-icons/vsc';
import {
  buildSidebarDragData,
  refreshMissingSidebarResourcePath,
} from '@/features/application/sidebar';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import { findResourceNodeSpawnTemplate } from '@/features/application/editor/canvasDrop';
import { RESOURCE_CATALOG_REFRESH_MESSAGE } from '@/features/application/editor/editorMutationAvailability';
import { openDatabaseEditorWindow } from '@/features/application/window';
import { focusDetail } from '@/features/core/editor/detail/detailFocusCommands';
import { TYPE_ICON_COLORS } from '@/features/domain/sidebar';
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
  const loadError = (data as { loadError?: unknown }).loadError;
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
    logger.notify.warn(RESOURCE_CATALOG_REFRESH_MESSAGE, "UI");
  };

  return (
    <SidebarListItem
      id={id}
      dragData={dragData}
      dragDisabledReason={RESOURCE_CATALOG_REFRESH_MESSAGE}
      onDisabledDragAttempt={handleDisabledDragAttempt}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={<VscDatabase size={SIDEBAR_ROW_ICON_SIZE} style={{ color: TYPE_ICON_COLORS.data }} />}
      label={name}
      onClick={(e) => {
        e.stopPropagation();
        focusDetail({ kind: 'data', id });
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
          {!isLoading && typeof loadError === 'string' && loadError.length > 0 && (
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
