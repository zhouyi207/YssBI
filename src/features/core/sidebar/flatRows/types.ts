import type { MouseEvent } from "react";

export const SIDEBAR_FLAT_ROW_HEIGHT = 28 as const;

export interface SidebarDatabaseItemRow {
  kind: "database";
  rowKey: string;
  level: number;
  id: string;
  resourcePath?: string;
  name: string;
  data: unknown;
}

export type SidebarItemRow = SidebarDatabaseItemRow;

export type SidebarSectionActionConfig = {
  onAdd?: () => void;
  addAriaLabel?: string;
  onHeaderContextMenu?: (e: MouseEvent) => void;
  onContentContextMenu?: (e: MouseEvent) => void;
};
