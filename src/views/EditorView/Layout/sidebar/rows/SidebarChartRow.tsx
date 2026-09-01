import { memo } from "react";
import { useTranslation } from "react-i18next";
import { VscGraphLine } from "react-icons/vsc";
import { TYPE_ICON_COLORS } from "@/features/domain/sidebar";
import { SidebarListItem, SidebarRowActionButton, SIDEBAR_ROW_ICON_SIZE } from "../../sidebarUi";

export const SidebarChartRow = memo(function SidebarChartRow({
  chartPath,
  name,
  indentDepth = 0,
  isSelected = false,
  onOpen,
  onContextMenu,
}: {
  chartPath: string;
  name: string;
  indentDepth?: number;
  isSelected?: boolean;
  onOpen: (chartPath: string, name: string) => void;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();

  return (
    <SidebarListItem
      id={chartPath}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={<VscGraphLine size={SIDEBAR_ROW_ICON_SIZE} style={{ color: TYPE_ICON_COLORS.chart }} />}
      label={name}
      onClick={(e) => {
        e.stopPropagation();
        void onOpen(chartPath, name);
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        void onOpen(chartPath, name);
      }}
      onContextMenu={onContextMenu}
      trailing={
        <SidebarRowActionButton
          isSelected={isSelected}
          tooltip={t("sidebar.open")}
          onClick={(e) => {
            e.stopPropagation();
            void onOpen(chartPath, name);
          }}
        />
      }
    />
  );
});
