import { memo } from 'react';
import type { SidebarRenderRow } from './sidebarRenderRows';
import { SidebarDataRow } from '../rows/SidebarDataRow';
import { SidebarGraphRow } from '../rows/SidebarGraphRow';
import { SidebarGroupRow } from '../rows/SidebarGroupRow';
import { SidebarNodeRow } from '../rows/SidebarNodeRow';
import { SidebarVariableRow } from '../rows/SidebarVariableRow';
import { SidebarWorksheetRow } from '../rows/SidebarWorksheetRow';
import { useSidebarFlatRowContext } from './sidebarFlatRowContext';
import { SidebarSectionEmptyState } from './SidebarSectionEmptyState';

export const SidebarFlatRowItem = memo(function SidebarFlatRowItem({
  row,
}: {
  row: SidebarRenderRow;
}) {
  const ctx = useSidebarFlatRowContext();

  switch (row.kind) {
    case 'section': {
      const actions = ctx.sectionActions[row.sectionKey];
      return (
        <SidebarGroupRow
          level={row.level}
          label={row.label}
          expanded={row.expanded}
          onToggle={() => ctx.onToggleSection(row.sectionKey)}
          onAdd={actions?.onAdd}
          addAriaLabel={actions?.addAriaLabel}
          onContextMenu={actions?.onHeaderContextMenu}
        />
      );
    }
    case 'group':
      return (
        <SidebarGroupRow
          level={row.level}
          label={row.label}
          expanded={row.expanded}
          onToggle={() => ctx.onToggleGroup(row.groupKey)}
        />
      );
    case 'sectionEmpty':
      return (
        <SidebarSectionEmptyState
          level={row.level}
          message={row.message}
          onContextMenu={ctx.sectionActions[row.sectionKey]?.onContentContextMenu}
        />
      );
    case 'graph':
      return (
        <SidebarGraphRow
          id={row.id}
          name={row.name}
          graphType={row.graphType}
          indentDepth={row.level}
          isSelected={ctx.detailTarget?.kind === row.graphType && ctx.detailTarget.path === row.id}
          issueCount={ctx.graphIssueCounts[row.id] ?? 0}
          onContextMenu={(e) =>
            ctx.onGraphContextMenu?.(e, {
              type: 'graph',
              id: row.id,
              name: row.name,
              graphType: row.graphType,
            })
          }
        />
      );
    case 'variable':
      return (
        <SidebarVariableRow
          id={row.id}
          name={row.name}
          dataType={row.dataType}
          isGlobal={row.isGlobal}
          indentDepth={row.level}
          isSelected={ctx.detailTarget?.kind === 'variable' && ctx.detailTarget.id === row.id}
          onContextMenu={(e) => ctx.onVariableContextMenu?.(e, row.id, row.name)}
        />
      );
    case 'database':
      return (
        <SidebarDataRow
          id={row.id}
          name={row.name}
          data={row.data}
          indentDepth={row.level}
          isSelected={ctx.detailTarget?.kind === 'data' && ctx.detailTarget.id === row.id}
          onContextMenu={(e) => ctx.onDatabaseContextMenu?.(e, row.id, row.name)}
        />
      );
    case 'worksheet':
      return (
        <SidebarWorksheetRow
          id={row.id}
          name={row.name}
          indentDepth={row.level}
          isSelected={ctx.detailTarget?.kind === 'worksheet' && ctx.detailTarget.id === row.id}
          onOpen={ctx.onOpenWorksheet ?? (() => undefined)}
          onContextMenu={(e) => ctx.onWorksheetContextMenu?.(e, row.id, row.name)}
        />
      );
    case 'node':
      return (
        <SidebarNodeRow
          item={row.item}
          level={row.level}
          selected={ctx.selectedNodeType === row.item.nodeType}
          onClick={() => ctx.onNodeLeafClick?.(row.item)}
        />
      );
    default: {
      const _exhaustive: never = row;
      return _exhaustive;
    }
  }
});
