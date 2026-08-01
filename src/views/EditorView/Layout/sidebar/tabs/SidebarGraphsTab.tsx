import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  buildGraphsSidebarModel,
  useSidebarSectionExpandSnapshot,
  useSidebarStore,
} from '@/features/core/sidebar';
import { useCallFunctionIssueCountsByGraph } from '@/features/application/graphDiagnostics/useCallFunctionDiagnostics';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';
import { SidebarFlatRowPanel } from '../sections/SidebarFlatRowPanel';
import { noopSidebarHandler } from '../sections/sidebarFlatRowContext';
import type { GraphResourceType } from '../../sidebarContextMenu';

export function SidebarGraphsTab({
  events,
  functions,
  onAddEvent,
  onAddFunction,
  onOpenContextMenu,
  onGraphContextMenu,
}: {
  events: Record<string, { name: string }>;
  functions: Record<string, { name: string }>;
  onAddEvent: () => void;
  onAddFunction: () => void;
  onOpenContextMenu: (e: React.MouseEvent, target: { type: 'section'; graphType: GraphResourceType }) => void;
  onGraphContextMenu: (
    e: React.MouseEvent,
    target: { type: 'graph'; id: string; name: string; graphType: GraphResourceType },
  ) => void;
}) {
  const { t } = useTranslation();
  const sectionExpanded = useSidebarSectionExpandSnapshot('graphsEvent', 'graphsFunction');
  const toggleSection = useSidebarStore((s) => s.toggleSection);
  const graphIssueCounts = useCallFunctionIssueCountsByGraph();

  const model = useMemo(
    () =>
      buildGraphsSidebarModel({
        events,
        functions,
        expandedSections: sectionExpanded,
        labels: {
          event: t('sidebar.sections.event'),
          function: t('sidebar.sections.function'),
          noEvents: t('sidebar.noEvents'),
          noFunctions: t('sidebar.noFunctions'),
        },
      }),
    [events, functions, sectionExpanded, t],
  );

  const sectionActions = useMemo(
    () => ({
      graphsEvent: {
        onAdd: onAddEvent,
        addAriaLabel: t('canvas.newEventGraph'),
        onHeaderContextMenu: (e: React.MouseEvent) => onOpenContextMenu(e, { type: 'section', graphType: 'event' }),
        onContentContextMenu: (e: React.MouseEvent) => onOpenContextMenu(e, { type: 'section', graphType: 'event' }),
      },
      graphsFunction: {
        onAdd: onAddFunction,
        addAriaLabel: t('canvas.newFunction'),
        onHeaderContextMenu: (e: React.MouseEvent) => onOpenContextMenu(e, { type: 'section', graphType: 'function' }),
        onContentContextMenu: (e: React.MouseEvent) => onOpenContextMenu(e, { type: 'section', graphType: 'function' }),
      },
    }),
    [onAddEvent, onAddFunction, onOpenContextMenu, t],
  );

  return (
    <SidebarTabPanel>
      <SidebarFlatRowPanel
        model={model}
        sectionActions={sectionActions}
        graphIssueCounts={graphIssueCounts}
        onToggleSection={toggleSection}
        onToggleGroup={noopSidebarHandler}
        onGraphContextMenu={onGraphContextMenu}
      />
    </SidebarTabPanel>
  );
}
