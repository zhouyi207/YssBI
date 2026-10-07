import { produce } from "immer";
import type { GraphConstantDto } from "@/shared/types/domain/editorMutation";
import { shareProjection } from "@/features/core/state/readProjection";
import { makeGraphConstantFixture } from "@/tests/helpers/editorProjectionFixtures";

type GraphConstants = Record<string, GraphConstantDto> | undefined;

export function makeDynamicKeyConstantFixture(id: string) {
  const constant = makeGraphConstantFixture(id);
  return {
    ...constant,
    dataValue: {
      Object: {
        ...constant.dataValue.Object,
        metadata: {
          Object: {
            ...constant.dataValue.Object.metadata.Object,
            ["__proto__"]: { List: [{ Integer: "40" }, { Integer: "41" }] },
            constructor: { Object: { keep: { String: "kept" }, change: { String: "old" } } },
          },
        },
      },
    },
  } satisfies GraphConstantDto;
}

export function changeDynamicKeyConstantFixture(
  constant: ReturnType<typeof makeDynamicKeyConstantFixture>,
) {
  constant.dataValue.Object.left.List[0].Integer = "999";
  const metadata = constant.dataValue.Object.metadata.Object;
  metadata.__proto__.List.reverse();
  metadata.constructor.Object.change.String = "new";
  delete (metadata as Partial<typeof metadata>).enabled;
  Object.assign(metadata, { added: { String: "added" } });
}

/** Restore ordinary JSON branches inside the caller's one Immer transaction. */
export function restoreEqualBranches(
  previous: unknown,
  next: unknown,
  draft: () => unknown,
  replace: (value: unknown) => void,
): boolean {
  if (Object.is(previous, next)) return true;
  if (
    previous === null ||
    next === null ||
    typeof previous !== "object" ||
    typeof next !== "object"
  )
    return false;
  if (Array.isArray(previous) && Array.isArray(next)) {
    let equal = previous.length === next.length;
    next.forEach((value, index) => {
      if (
        restoreEqualBranches(
          previous[index],
          value,
          () => (draft() as unknown[])[index],
          (shared) => {
            (draft() as unknown[])[index] = shared;
          },
        )
      ) {
        if (!Object.is(previous[index], value)) (draft() as unknown[])[index] = previous[index];
      } else equal = false;
    });
    return equal;
  }
  if (
    Object.getPrototypeOf(previous) !== Object.prototype ||
    Object.getPrototypeOf(next) !== Object.prototype
  )
    return false;
  const before = previous as Record<string, unknown>;
  const after = next as Record<string, unknown>;
  // Immer's setter dispatch reaches Object.prototype.__proto__ even for an own key.
  // Decide before touching this draft subtree; its parent path is a safe field/index.
  if (Object.prototype.hasOwnProperty.call(after, "__proto__")) {
    const shared = shareProjection(before, after);
    if (shared === before) return true;
    if (shared !== after) replace(shared);
    return false;
  }
  const keys = Object.keys(after);
  let equal = keys.length === Object.keys(before).length;
  for (const key of keys) {
    const shared = restoreEqualBranches(
      before[key],
      after[key],
      () => (draft() as Record<string, unknown>)[key],
      (shared) => {
        (draft() as Record<string, unknown>)[key] = shared;
      },
    );
    if (!Object.prototype.hasOwnProperty.call(before, key) || !shared) equal = false;
    if (shared && !Object.is(before[key], after[key]))
      (draft() as Record<string, unknown>)[key] = before[key];
  }
  return equal;
}

/** Benchmark reference: one Immer transaction, with the existing helper at unsafe dictionaries. */
export function shareGraphConstantsWithImmerReference(
  previous: GraphConstants,
  next: GraphConstants,
): GraphConstants {
  if (!previous || !next || previous === next) return next;
  let equal = false;
  let replacement: GraphConstants;
  const shared = produce(next, (draft) => {
    equal = restoreEqualBranches(
      previous,
      next,
      () => draft,
      (value) => {
        replacement = value as GraphConstants;
      },
    );
  });
  // A fresh equal snapshot keeps the published root. Already shared deltas cause no
  // draft writes and keep the incoming root, including its validation/freeze proofs.
  return equal ? previous : (replacement ?? shared);
}
