import { resolveSectionExpanded, type SidebarSectionKey } from '../sidebarSectionState';
import type { FlatSidebarRow } from './types';

/** Append one persisted section header and its visible child rows. */
export function appendSectionBlock(
  rows: FlatSidebarRow[],
  params: {
    sectionKey: SidebarSectionKey;
    label: string;
    expandedSections: Record<string, boolean>;
    emptyMessage?: string;
    itemRows: FlatSidebarRow[];
  },
): void {
  const expanded = resolveSectionExpanded(params.expandedSections, params.sectionKey);
  rows.push({
    kind: 'section',
    rowKey: `section:${params.sectionKey}`,
    sectionKey: params.sectionKey,
    level: 0,
    label: params.label,
    expanded,
  });

  if (!expanded) return;

  if (params.itemRows.length === 0 && params.emptyMessage) {
    rows.push({
      kind: 'empty',
      rowKey: `empty:${params.sectionKey}`,
      level: 1,
      message: params.emptyMessage,
      sectionKey: params.sectionKey,
    });
    return;
  }

  rows.push(...params.itemRows);
}
