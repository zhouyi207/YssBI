import { produce } from "immer";
import type { GraphConstantDto } from "@/shared/types/domain/editorMutation";
import { shareProjection } from "@/features/core/state/readProjection";
import { restoreValueType } from "./valueTypeProjection";

type GraphConstants = Record<string, GraphConstantDto> | undefined;

function shareTabular(
  previous: GraphConstantDto["tabular"],
  next: GraphConstantDto["tabular"],
): GraphConstantDto["tabular"] {
  if (
    !previous ||
    !next ||
    previous === next ||
    Object.getPrototypeOf(previous) !== Object.prototype ||
    Object.getPrototypeOf(next) !== Object.prototype
  )
    return next;
  if (previous.columns === next.columns) return previous;
  if (
    Object.getPrototypeOf(previous.columns) !== Object.prototype ||
    Object.getPrototypeOf(next.columns) !== Object.prototype
  )
    return next;
  const keys = Object.keys(next.columns);
  let equal = keys.length === Object.keys(previous.columns).length;
  let columns = next.columns;
  for (const key of keys) {
    const cells = next.columns[key];
    const before = Object.prototype.hasOwnProperty.call(previous.columns, key)
      ? previous.columns[key]
      : undefined;
    if (
      !before ||
      before.length !== cells.length ||
      !cells.every((cell, index) => Object.is(cell, before[index]))
    ) {
      equal = false;
    } else if (before !== cells) {
      // Every assigned key already exists as an own data property on this plain copy.
      if (columns === next.columns) columns = { ...next.columns };
      columns[key] = before;
    }
  }
  return equal ? previous : columns === next.columns ? next : { ...next, columns };
}

/** Share validated incoming constants; the Rust document remains the only write owner. */
export function shareGraphConstants(
  previous: GraphConstants,
  next: GraphConstants,
): GraphConstants {
  if (
    !previous ||
    !next ||
    previous === next ||
    Object.getPrototypeOf(previous) !== Object.prototype ||
    Object.getPrototypeOf(next) !== Object.prototype
  )
    return next;
  const ids = Object.keys(next);
  let equal = ids.length === Object.keys(previous).length;
  const shared = produce(next, (draft) => {
    for (const id of ids) {
      const constant = next[id];
      const before = previous[id];
      if (before === constant) continue;
      if (
        !before ||
        Object.getPrototypeOf(before) !== Object.prototype ||
        Object.getPrototypeOf(constant) !== Object.prototype
      ) {
        equal = false;
        continue;
      }
      const dataTypeEqual = restoreValueType(
        before.dataType,
        constant.dataType,
        () => draft[id].dataType,
      );
      // Compare unrestricted JSON and column names outside the draft; assign only DTO fields.
      const dataValue = shareProjection(before.dataValue, constant.dataValue);
      const tabular = shareTabular(before.tabular, constant.tabular);
      const tags =
        before.tags?.length === constant.tags?.length &&
        constant.tags?.every((tag, index) => tag === before.tags![index])
          ? before.tags
          : constant.tags;
      if (
        before.id === constant.id &&
        before.name === constant.name &&
        before.description === constant.description &&
        dataTypeEqual &&
        before.dataValue === dataValue &&
        before.tabular === tabular &&
        before.tags === tags &&
        Object.keys(before).length === Object.keys(constant).length
      ) {
        draft[id] = before;
        continue;
      }
      equal = false;
      if (dataTypeEqual && before.dataType !== constant.dataType)
        draft[id].dataType = before.dataType;
      if (dataValue !== constant.dataValue) draft[id].dataValue = dataValue;
      if (tabular !== constant.tabular) draft[id].tabular = tabular;
      if (tags !== constant.tags) draft[id].tags = tags;
    }
  });
  // Prefer the published root for full equality, then preserve an already-shared
  // incoming delta's identity when no draft assignments were needed.
  return equal ? previous : shared;
}
