import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import {
  buildChartsSidebarModel,
  useSidebarSectionExpandSnapshot,
  useSidebarStore,
} from '@/features/core/sidebar';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';
import { SidebarFlatRowPanel } from '../sections/SidebarFlatRowPanel';
import { noopSidebarHandler } from '../sections/sidebarFlatRowContext';

export function SidebarChartsTab({
  onAddWorksheet,
  onOpenWorksheet,
  onSectionContextMenu,
  onWorksheetContextMenu,
}: {
  onAddWorksheet: () => void;
  onOpenWorksheet: (id: string, name: string) => void;
  onSectionContextMenu: (e: React.MouseEvent) => void;
  onWorksheetContextMenu: (e: React.MouseEvent, id: string, name: string) => void;
}) {
  const { t } = useTranslation();
  const worksheets = useWorksheetStore((s) => s.index);
  const sectionExpanded = useSidebarSectionExpandSnapshot('chartsWorksheets');
  const toggleSection = useSidebarStore((s) => s.toggleSection);

  const model = useMemo(
    () =>
      buildChartsSidebarModel({
        worksheets,
        expandedSections: sectionExpanded,
        labels: {
          worksheets: t('chartsSidebar.worksheets'),
          noWorksheets: t('chartsSidebar.noWorksheets'),
        },
      }),
    [sectionExpanded, t, worksheets],
  );

  const sectionActions = useMemo(
    () => ({
      chartsWorksheets: {
        onAdd: onAddWorksheet,
        addAriaLabel: t('contextMenu.sidebar.newWorksheet'),
        onHeaderContextMenu: onSectionContextMenu,
        onContentContextMenu: onSectionContextMenu,
      },
    }),
    [onAddWorksheet, onSectionContextMenu, t],
  );

  return (
    <SidebarTabPanel>
      <SidebarFlatRowPanel
        model={model}
        sectionActions={sectionActions}
        onToggleSection={toggleSection}
        onToggleGroup={noopSidebarHandler}
        onOpenWorksheet={onOpenWorksheet}
        onWorksheetContextMenu={onWorksheetContextMenu}
      />
    </SidebarTabPanel>
  );
}
