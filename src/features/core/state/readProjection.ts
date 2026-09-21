import { useMemo } from "react";
import { useStoreWithEqualityFn } from "zustand/traditional";
import { shallow } from "zustand/shallow";
import { freezePublishedValue, type DeepReadonly } from "@/shared/types/deepReadonly";

export interface ReadProjection<T> {
  getSnapshot(): T;
  subscribe(listener: () => void): () => void;
}

/** Module-lifetime projection of existing owners, with no writable store or read-time cloning. */
export function createReadProjection<T extends object>(
  project: () => T,
  sources: readonly Pick<ReadProjection<unknown>, "subscribe">[],
): ReadProjection<DeepReadonly<T>> {
  let snapshot = freezePublishedValue(project());
  const listeners = new Set<() => void>();
  const refresh = () => {
    const next = project();
    if (shallow<unknown>(snapshot, next)) return;
    snapshot = freezePublishedValue(next);
    for (const listener of Array.from(listeners)) listener();
  };
  for (const source of sources) source.subscribe(refresh);
  return {
    getSnapshot: () => snapshot,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export function useReadProjection<T, S>(port: ReadProjection<T>, selector: (snapshot: T) => S): S {
  const api = useMemo(
    () => ({
      getState: port.getSnapshot,
      getInitialState: port.getSnapshot,
      subscribe: (listener: (next: T, previous: T) => void) => {
        let previous = port.getSnapshot();
        return port.subscribe(() => {
          const next = port.getSnapshot();
          const before = previous;
          previous = next;
          listener(next, before);
        });
      },
    }),
    [port],
  );
  return useStoreWithEqualityFn(api, selector);
}

/** Small JSON read projections reuse equal branches; native layout and business stores own writes. */
export function shareProjection<T>(previous: T | undefined, next: T): T {
  if (Object.is(previous, next)) return next;
  if (
    previous === null ||
    next === null ||
    typeof previous !== "object" ||
    typeof next !== "object"
  )
    return next;
  if (Array.isArray(previous) && Array.isArray(next)) {
    const values = next.map((value, index) => shareProjection(previous[index], value));
    return (
      previous.length === values.length &&
      values.every((value, index) => Object.is(value, previous[index]))
        ? previous
        : values
    ) as T;
  }
  if (
    Object.getPrototypeOf(previous) !== Object.prototype ||
    Object.getPrototypeOf(next) !== Object.prototype
  )
    return next;
  const before = previous as Record<string, unknown>;
  const after = next as Record<string, unknown>;
  const keys = Object.keys(after);
  let equal = keys.length === Object.keys(before).length;
  const values = Object.fromEntries(
    keys.map((key) => {
      const value = shareProjection(before[key], after[key]);
      if (!Object.prototype.hasOwnProperty.call(before, key) || !Object.is(value, before[key]))
        equal = false;
      return [key, value];
    }),
  );
  return (equal ? previous : values) as T;
}
