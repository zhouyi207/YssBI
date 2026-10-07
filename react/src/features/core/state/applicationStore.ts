import { useStoreWithEqualityFn } from "zustand/traditional";
import { createStore, type StoreApi } from "zustand/vanilla";

type StoreUpdate<T> = Partial<T> | T | ((state: T) => Partial<T> | T);

export type BoundApplicationStore<T> = StoreApi<T> & {
  <U = T>(selector?: (state: T) => U): U;
};

/**
 * Binds a vanilla store to React without exposing a second application-level
 * state library entry point. The imperative StoreApi is retained for event
 * and IPC projection updates; views consume the hook only through Application.
 */
export function createBoundApplicationStore<T>(
  initializer: (set: (update: StoreUpdate<T>, replace?: boolean) => void, get: () => T) => T,
): BoundApplicationStore<T> {
  const store = createStore<T>((set, get) =>
    initializer((update, replace = false) => {
      const apply = set as unknown as (
        next: T | Partial<T> | ((state: T) => T | Partial<T>),
        replace?: boolean,
      ) => void;
      apply(update, replace);
    }, get),
  );

  const useBoundStore = (<U = T>(selector?: (state: T) => U): U => {
    const select = selector ?? ((state: T) => state as unknown as U);
    return useStoreWithEqualityFn(store, select);
  }) as BoundApplicationStore<T>;

  return Object.assign(useBoundStore, store);
}
