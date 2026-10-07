import type { Draft } from "immer";
import type { ValueType } from "@/shared/types/domain/valueType";

/** Restore published branches inside the caller's incoming-value Immer transaction. */
export function restoreValueType(
  previous: ValueType | null,
  next: ValueType | null,
  draft: () => Draft<ValueType>,
): boolean {
  if (previous === next) return true;
  if (!previous || !next) return false;
  if (
    (previous.kind === "Array" || previous.kind === "DataSeries") &&
    (next.kind === "Array" || next.kind === "DataSeries")
  ) {
    const innerEqual = restoreValueType(previous.inner, next.inner, () => {
      const target = draft() as Draft<typeof next>;
      return target.inner;
    });
    if (previous.kind === next.kind && innerEqual) return true;
    if (innerEqual && previous.inner !== next.inner) {
      const target = draft() as Draft<typeof next>;
      target.inner = previous.inner;
    }
    return false;
  }
  if (previous.kind === "OneOf" && next.kind === "OneOf") {
    return restoreValueTypes(previous.inner, next.inner, () => {
      const target = draft() as Draft<typeof next>;
      return target.inner;
    });
  }
  if (previous.kind !== next.kind) return false;
  if (previous.kind === "Scalar" && next.kind === "Scalar") return previous.inner === next.inner;
  if (previous.kind === "Struct" && next.kind === "Struct") return previous.inner === next.inner;
  return next.kind === "Object" || next.kind === "Any" || next.kind === "DataFrame";
}

export function restoreValueTypes(
  previous: ValueType[],
  next: ValueType[],
  draft: () => Draft<ValueType[]>,
): boolean {
  if (previous === next) return true;
  let equal = previous.length === next.length;
  for (let index = 0; index < next.length; index++) {
    const before = previous[index];
    const type = next[index];
    if (before && restoreValueType(before, type, () => draft()[index])) {
      if (before !== type) draft()[index] = before;
    } else equal = false;
  }
  return equal;
}
