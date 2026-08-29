import type { SidebarItemRow, SidebarPanelModel } from '@/features/core/sidebar/flatRows';
import type { SidebarSectionKey } from '@/features/core/sidebar';

export type SidebarRenderRow =
  | SidebarItemRow
  | {
      kind: 'section';
      rowKey: string;
      sectionKey: SidebarSectionKey;
      level: 0;
      label: string;
      expanded: boolean;
    }
  | {
      kind: 'sectionEmpty';
      rowKey: string;
      sectionKey: SidebarSectionKey;
      level: 1;
      message: string;
    };

export function flattenSidebarPanelModel(model: SidebarPanelModel): SidebarRenderRow[] {
  return model.sections.flatMap<SidebarRenderRow>((section) => {
    const header: SidebarRenderRow = {
      kind: 'section',
      rowKey: `section:${section.key}`,
      sectionKey: section.key,
      level: 0,
      label: section.label,
      expanded: section.expanded,
    };
    if (!section.expanded) return [header];
    if (section.rows.length > 0) return [header, ...section.rows];
    if (!section.emptyMessage) return [header];
    return [
      header,
      {
        kind: 'sectionEmpty',
        rowKey: `section-empty:${section.key}`,
        sectionKey: section.key,
        level: 1,
        message: section.emptyMessage,
      },
    ];
  });
}
