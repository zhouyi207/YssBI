import { memo } from "react";
import { useTranslation } from "react-i18next";
import { VscSymbolVariable } from "react-icons/vsc";
import {
  buildSidebarDragData,
  refreshMissingSidebarResourcePath,
} from "@/features/application/sidebar";
import { useLocalizedNodeCatalog } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { findResourceNodeSpawnTemplate } from "@/features/application/editor/canvasDrop";
import { revealDetails } from "@/features/application/editor/rightSidebarActions";
import { TYPE_ICON_COLORS } from "@/features/domain/sidebar";
import type { DataType } from "@/shared/types/domain/dataType";
import { safeDataTypeColor, safeDataTypeDisplay } from "../../sidebarUtils";
import {
  SidebarListItem,
  sidebarVariableTypeBadgeClass,
  SIDEBAR_ROW_ICON_SIZE,
} from "../../sidebarUi";

export const SidebarVariableRow = memo(function SidebarVariableRow({
  id,
  resourcePath,
  name,
  dataType,
  isGlobal,
  indentDepth = 0,
  isSelected = false,
  onContextMenu,
}: {
  id: string;
  resourcePath?: string;
  name: string;
  dataType: unknown;
  isGlobal: boolean;
  indentDepth?: number;
  isSelected?: boolean;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();
  const iconColor = isGlobal ? TYPE_ICON_COLORS.variableGlobal : TYPE_ICON_COLORS.variable;
  const { status, catalog, refresh } = useLocalizedNodeCatalog();
  const templateForPath = (path: string) =>
    status === "ready" && catalog
      ? findResourceNodeSpawnTemplate(catalog.items, path, "variable", "yssbi.project.variable.get")
      : null;
  const template = resourcePath ? templateForPath(resourcePath) : null;
  const dragData = template
    ? buildSidebarDragData(id, name, "variable", template.descriptor)
    : null;
  const resourceCatalogRefreshMessage = t("notifications.editor.resourceCatalogRefreshing");
  const handleDisabledDragAttempt = () => {
    if (resourcePath) {
      refresh();
    } else {
      void refreshMissingSidebarResourcePath({
        kind: "variable",
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
      icon={<VscSymbolVariable size={SIDEBAR_ROW_ICON_SIZE} style={{ color: iconColor }} />}
      label={name}
      onClick={async (e) => {
        e.stopPropagation();
        await revealDetails({ kind: "variable", id });
      }}
      onContextMenu={onContextMenu}
      trailing={
        <span
          className={sidebarVariableTypeBadgeClass(isSelected)}
          style={{ color: safeDataTypeColor(dataType) }}
        >
          {safeDataTypeDisplay(dataType)}
          {dataType &&
          typeof dataType === "object" &&
          "kind" in dataType &&
          (dataType as DataType).kind === "Array" ? (
            <span className="text-[8px]">[]</span>
          ) : null}
        </span>
      }
    />
  );
});
