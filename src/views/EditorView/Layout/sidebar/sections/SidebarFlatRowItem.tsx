import { memo } from 'react';
import type { SidebarRenderRow } from './sidebarRenderRows';
import { SidebarDataRow } from '../rows/SidebarDataRow';
import { SidebarGroupRow } from '../rows/SidebarGroupRow';
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
    case 'sectionEmpty':
      return (
        <SidebarSectionEmptyState
          level={row.level}
          message={row.message}
          onContextMenu={ctx.sectionActions[row.sectionKey]?.onContentContextMenu}
        />
      );
    case 'database':
      return (
        <SidebarDataRow
          id={row.id}
          resourcePath={row.resourcePath}
          name={row.name}
          data={row.data}
          indentDepth={row.level}
          isSelected={ctx.detailTarget?.kind === 'data' && ctx.detailTarget.id === row.id}
          onContextMenu={(e) => ctx.onDatabaseContextMenu?.(e, row.id, row.name)}
        />
      );
    default: {
      const _exhaustive: never = row;
      return _exhaustive;
    }
  }
});
