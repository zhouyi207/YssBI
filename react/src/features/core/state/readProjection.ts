import { useMemo } from "react";
import { useStoreWithEqualityFn } from "zustand/traditional";
import { createStore } from "zustand/vanilla";
import { shallow } from "zustand/shallow";
import { freezePublishedValue, type DeepReadonly } from "@/shared/types/deepReadonly";

export interface ReadProjection<T> {
  getSnapshot(): T;
  subscribe(listener: () => void): () => void;
}

/** Module-lifetime projection of existing owners, with no public writes or read-time cloning. */
export function createReadProjection<T extends object>(
  project: () => T,
  sources: readonly Pick<ReadProjection<unknown>, "subscribe">[],
): ReadProjection<DeepReadonly<T>> {
  // Zustand retains its initial state. Start empty so that it does not retain the first
  // graph/resource snapshot after the owner replaces it or closes the project.
  const projection = createStore<DeepReadonly<T> | undefined>(() => undefined);
  const refresh = () => {
    const next = project();
    if (shallow<unknown>(projection.getState(), next)) return;
    projection.setState(freezePublishedValue(next), true);
  };
  // Initialize before exposing readers or subscribing to the sources.
  refresh();
  for (const source of sources) source.subscribe(refresh);
  return {
    getSnapshot: projection.getState as () => DeepReadonly<T>,
    subscribe: projection.subscribe,
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
    let equal = previous.length === next.length;
    let values: unknown[] | undefined;
    next.forEach((value, index) => {
      const shared = shareProjection(previous[index], value);
      if (!Object.is(shared, previous[index])) equal = false;
      if (!Object.is(shared, value)) (values ??= next.slice())[index] = shared;
    });
    return (equal ? previous : (values ?? next)) as T;
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
  let values: Record<string, unknown> | undefined;
  for (const key of keys) {
    const value = shareProjection(before[key], after[key]);
    if (!Object.prototype.hasOwnProperty.call(before, key) || !Object.is(value, before[key]))
      equal = false;
    if (!Object.is(value, after[key])) (values ??= { ...after })[key] = value;
  }
  // Delta inputs may already share every unchanged child. Keep their immutable identity
  // so publication and validation can reuse the same objects as the transport baseline.
  return (equal ? previous : (values ?? next)) as T;
}
