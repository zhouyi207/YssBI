import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { buildDataSidebarModel } from "@/features/core/sidebar/flatRows";
import { sidebarUi, useSidebarUi } from "@/features/core/sidebar/ui";
import { SidebarTabPanel } from "@/modules/workbench/public";
import { SidebarFlatRowPanel } from "./SidebarFlatRowPanel";

export function SidebarDataTab({
  dataframes,
  onImport,
  onSectionContextMenu,
  onDatabaseContextMenu,
}: {
  dataframes: Readonly<Record<string, { readonly name: string; readonly resourcePath?: string }>>;
  onImport: () => void;
  onSectionContextMenu: (e: React.MouseEvent) => void;
  onDatabaseContextMenu: (e: React.MouseEvent, id: string, name: string) => void;
}) {
  const { t } = useTranslation();
  const sectionExpanded = useSidebarUi((snapshot) => ({
    dataData: snapshot.expandedSections.dataData ?? true,
  }));
  const toggleSection = sidebarUi.toggleSection;

  const model = useMemo(
    () =>
      buildDataSidebarModel({
        dataframes: dataframes ?? {},
        expandedSections: sectionExpanded,
        labels: {
          data: t("sidebar.sections.data"),
          noData: t("sidebar.noData"),
        },
      }),
    [dataframes, sectionExpanded, t],
  );

  const sectionActions = useMemo(
    () => ({
      dataData: {
        onAdd: onImport,
        addAriaLabel: t("contextMenu.sidebar.importData"),
        onHeaderContextMenu: onSectionContextMenu,
        onContentContextMenu: onSectionContextMenu,
      },
    }),
    [onImport, onSectionContextMenu, t],
  );

  return (
    <SidebarTabPanel>
      <SidebarFlatRowPanel
        model={model}
        sectionActions={sectionActions}
        onToggleSection={toggleSection}
        onDatabaseContextMenu={onDatabaseContextMenu}
      />
    </SidebarTabPanel>
  );
}
