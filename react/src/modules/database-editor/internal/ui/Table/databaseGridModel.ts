import type { DatabaseRow } from "@/shared/types/domain/database";
import type { DatabaseGridSelectionModifiers } from "@/features/domain/databaseEditor/gridSelection";

export interface DatabaseGridRow {
  values: DatabaseRow;
  rowId: string;
  sourceRowIndex: number;
}

const DATA_COLUMN_PREFIX = "data_";

export function dataColumnId(columnIndex: number): string {
  return `${DATA_COLUMN_PREFIX}${columnIndex}`;
}

export function dataColumnIndexFromId(columnId: string): number | null {
  if (!columnId.startsWith(DATA_COLUMN_PREFIX)) return null;
  const columnIndex = Number(columnId.slice(DATA_COLUMN_PREFIX.length));
  return Number.isInteger(columnIndex) && columnIndex >= 0 ? columnIndex : null;
}

export function selectionModifiers(event: {
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}): DatabaseGridSelectionModifiers {
  return {
    additive: event.ctrlKey || event.metaKey,
    extend: event.shiftKey,
  };
}
