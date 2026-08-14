export const WORKBENCH_ROOT_ID = 'root';
export const EDITOR_AREA_ID = 'editor';
export const DEFAULT_EDITOR_GROUP_ID = 'default_editor';
export const WORKBENCH_PART_IDS = ['sidebar', 'panel', 'detail'] as const;
export type WorkbenchPartId = (typeof WORKBENCH_PART_IDS)[number];
export const PANEL_PART_ID = 'panel';
export const DETAIL_PART_ID = 'detail';

export function isWorkbenchChromePartId(id: string): id is WorkbenchPartId {
  return (WORKBENCH_PART_IDS as readonly string[]).includes(id);
}
