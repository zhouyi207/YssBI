import { castDraft, isDraft, original, type Draft } from "immer";
import { shallow } from "zustand/shallow";
import type { ColumnInfo, DatabaseRecord } from "@/shared/types/domain/database";

/** Update validated metadata inside the resource owner's existing Immer transaction. */
export function updateDatabaseColumns(
  record: Draft<DatabaseRecord>,
  columns: readonly ColumnInfo[] | undefined,
): void {
  const previous = isDraft(record) ? original(record) : record;
  if (previous?.columns === columns) return;
  if (columns === undefined) {
    delete record.columns;
    return;
  }
  if (!record.columns) {
    record.columns = castDraft(structuredClone(columns));
    return;
  }

  record.columns.length = columns.length;
  for (const [index, column] of columns.entries()) {
    if (previous?.columns?.[index] === column) continue;
    const current = record.columns[index];
    if (!current) {
      record.columns[index] = castDraft(structuredClone(column));
      continue;
    }
    current.name = column.name;
    current.type = column.type;
    if ("physical" in column) current.physical = column.physical;
    else delete current.physical;
    if (!column.semantic || !current.semantic) {
      if ("semantic" in column) current.semantic = castDraft(structuredClone(column.semantic));
      else delete current.semantic;
      continue;
    }

    const semantic = current.semantic;
    semantic.kind = column.semantic.kind;
    semantic.positiveValue = column.semantic.positiveValue;
    if (!shallow(semantic.numeric, column.semantic.numeric))
      semantic.numeric = structuredClone(column.semantic.numeric);
    semantic.values.length = column.semantic.values.length;
    for (const [valueIndex, value] of column.semantic.values.entries()) {
      if (!shallow(semantic.values[valueIndex], value))
        semantic.values[valueIndex] = structuredClone(value);
    }
  }
}
