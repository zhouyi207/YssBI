import { memo } from "react";
import { useTranslation } from "react-i18next";
import { VscDatabase } from "react-icons/vsc";
import { buildSidebarDragData } from "@/features/application/sidebar";
import { useLocalizedNodeCatalog } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { findResourceNodeSpawnTemplate } from "@/features/application/editor/canvasDrop";
import { openDatabaseInEditor } from "@/features/application/editor/openDatabaseInEditor";
import { useDatabaseRead } from "@/features/core/database/read";
import { TYPE_ICON_COLORS } from "@/features/domain/sidebar";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  SidebarListItem,
  SidebarRowActionButton,
  SIDEBAR_ROW_ICON_SIZE,
} from "@/modules/workbench/public";

export const SidebarDataRow = memo(function SidebarDataRow({
  id,
  resourcePath,
  name,
  indentDepth = 0,
  isSelected = false,
  onContextMenu,
}: {
  id: string;
  resourcePath: string;
  name: string;
  indentDepth?: number;
  isSelected?: boolean;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();
  const loadFailed = useDatabaseRead((snapshot) => snapshot.databases[id]?.loadFailed === true);
  const { status, catalog, refresh } = useLocalizedNodeCatalog();
  const template =
    status === "ready" && catalog
      ? findResourceNodeSpawnTemplate(
          catalog.items,
          resourcePath,
          "database",
          "yssbi.dataframe.source.get",
        )
      : null;
  const dragData = template ? buildSidebarDragData(id, name, "data", template.descriptor) : null;
  const resourceCatalogRefreshMessage = t("notifications.editor.resourceCatalogRefreshing");

  return (
    <SidebarListItem
      id={id}
      dragData={dragData}
      dragDisabledReason={resourceCatalogRefreshMessage}
      onDisabledDragAttempt={refresh}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={<VscDatabase size={SIDEBAR_ROW_ICON_SIZE} style={{ color: TYPE_ICON_COLORS.data }} />}
      label={name}
      onClick={async (e) => {
        e.stopPropagation();
        await openDatabaseInEditor(id);
      }}
      onContextMenu={onContextMenu}
      trailing={
        <>
          {loadFailed && (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-red-500" />
              </TooltipTrigger>
              <TooltipContent side="top">{t("sidebar.dataLoadFailed")}</TooltipContent>
            </Tooltip>
          )}
          <SidebarRowActionButton
            isSelected={isSelected}
            tooltip={t("sidebar.viewInDatabaseEditor")}
            onClick={(e) => {
              e.stopPropagation();
              void openDatabaseInEditor(id);
            }}
          />
        </>
      }
    />
  );
});
