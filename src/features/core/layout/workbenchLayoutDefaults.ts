export const DEFAULT_EDITOR_GROUP_ID = 'default_editor';
export const WORKBENCH_PART_IDS = ['sidebar', 'panel', 'detail'] as const;
export type WorkbenchPartId = (typeof WORKBENCH_PART_IDS)[number];
