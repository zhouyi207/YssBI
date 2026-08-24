import { VscAdd } from 'react-icons/vsc';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import type { ProjectResourceBrowserRow } from '@/features/application/sidebar/projectResourceBrowser';
import type { DetailTarget } from '@/features/core/editor/detail/types';
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from '@/features/core/sidebar/projectTreeState';
import {
  SIDEBAR_ROW_ICON_SIZE,
  SidebarTreeCategoryRow,
} from '../../sidebarUi';
import type { GraphResourceType } from '../../sidebarContextMenu';
import { SidebarSectionEmptyState } from '../sections/SidebarSectionEmptyState';
import { SidebarGraphRow } from './SidebarGraphRow';
import { SidebarVariableRow } from './SidebarVariableRow';
import { SidebarWorksheetRow } from './SidebarWorksheetRow';

export interface SidebarProjectTreeActions {
  onAddEvent: () => void;
  onAddFunction: () => void;
  onAddWorksheet: () => void;
  onAddVariable: (isGlobal: boolean) => void;
  onCategoryContextMenu: (event: React.MouseEvent, categoryId: ProjectTreeCategoryId) => void;
  onGraphContextMenu: (
    event: React.MouseEvent,
    target: { type: 'graph'; id: string; name: string; graphType: GraphResourceType },
  ) => void;
  onVariableContextMenu: (event: React.MouseEvent, id: string, name: string) => void;
  onWorksheetContextMenu: (event: React.MouseEvent, path: string, name: string) => void;
  onOpenWorksheet: (path: string, name: string) => void;
}

function categoryAddConfig(
  categoryId: ProjectTreeCategoryId,
  actions: SidebarProjectTreeActions,
  t: ReturnType<typeof useTranslation>['t'],
): { onAdd: () => void; ariaLabel: string } | null {
  switch (categoryId) {
    case PROJECT_TREE_CATEGORY_IDS.events:
      return { onAdd: actions.onAddEvent, ariaLabel: t('canvas.newEventGraph') };
    case PROJECT_TREE_CATEGORY_IDS.functions:
      return { onAdd: actions.onAddFunction, ariaLabel: t('canvas.newFunction') };
    case PROJECT_TREE_CATEGORY_IDS.worksheets:
      return {
        onAdd: actions.onAddWorksheet,
        ariaLabel: t('contextMenu.sidebar.newWorksheet'),
      };
    case PROJECT_TREE_CATEGORY_IDS.variables:
      return null;
    case PROJECT_TREE_CATEGORY_IDS.localVariables:
      return {
        onAdd: () => actions.onAddVariable(false),
        ariaLabel: t('contextMenu.sidebar.newLocalVariable'),
      };
    case PROJECT_TREE_CATEGORY_IDS.globalVariables:
      return {
        onAdd: () => actions.onAddVariable(true),
        ariaLabel: t('contextMenu.sidebar.newGlobalVariable'),
      };
  }
}

export function SidebarProjectTreeRow({
  row,
  actions,
  detailTarget,
  graphIssueCounts,
  categoryInteractionDisabled,
  onCategoryExpandedChange,
}: {
  row: ProjectResourceBrowserRow;
  actions: SidebarProjectTreeActions;
  detailTarget: DetailTarget | null;
  graphIssueCounts: Record<string, number>;
  categoryInteractionDisabled: boolean;
  onCategoryExpandedChange: (categoryId: ProjectTreeCategoryId, expanded: boolean) => void;
}) {
  const { t } = useTranslation();

  switch (row.kind) {
    case 'category': {
      const addConfig = categoryAddConfig(row.categoryId, actions, t);
      return (
        <SidebarTreeCategoryRow
          categoryId={row.categoryId}
          label={row.label}
          depth={row.level}
          expanded={row.expanded}
          interactionDisabled={categoryInteractionDisabled}
          onExpandedChange={(expanded) => onCategoryExpandedChange(row.categoryId, expanded)}
          onContextMenu={(event) => actions.onCategoryContextMenu(event, row.categoryId)}
          trailing={addConfig ? (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={addConfig.ariaLabel}
              onClick={(event) => {
                event.stopPropagation();
                addConfig.onAdd();
              }}
              className="size-6 shrink-0 p-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
            >
              <VscAdd size={SIDEBAR_ROW_ICON_SIZE} />
            </Button>
          ) : undefined}
        />
      );
    }
    case 'empty': {
      return (
        <SidebarSectionEmptyState
          level={row.level}
          message={row.message}
          onContextMenu={(event) => actions.onCategoryContextMenu(event, row.categoryId)}
        />
      );
    }
    case 'graph':
      return (
        <SidebarGraphRow
          id={row.id}
          name={row.name}
          graphType={row.graphType}
          indentDepth={row.level}
          isSelected={detailTarget?.kind === row.graphType && detailTarget.path === row.id}
          issueCount={graphIssueCounts[row.id] ?? 0}
          onContextMenu={(event) => actions.onGraphContextMenu(event, {
            type: 'graph',
            id: row.id,
            name: row.name,
            graphType: row.graphType,
          })}
        />
      );
    case 'variable':
      return (
        <SidebarVariableRow
          id={row.id}
          resourcePath={row.resourcePath}
          name={row.name}
          dataType={row.dataType}
          isGlobal={row.isGlobal}
          indentDepth={row.level}
          isSelected={detailTarget?.kind === 'variable' && detailTarget.id === row.id}
          onContextMenu={(event) => actions.onVariableContextMenu(event, row.id, row.name)}
        />
      );
    case 'worksheet':
      return (
        <SidebarWorksheetRow
          worksheetPath={row.worksheetPath}
          name={row.name}
          indentDepth={row.level}
          isSelected={detailTarget?.kind === 'worksheet'
            && detailTarget.worksheetPath === row.worksheetPath}
          onOpen={actions.onOpenWorksheet}
          onContextMenu={(event) => actions.onWorksheetContextMenu(
            event,
            row.worksheetPath,
            row.name,
          )}
        />
      );
    default: {
      const _exhaustive: never = row;
      return _exhaustive;
    }
  }
}
