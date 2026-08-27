export type DeepReadonly<T> = T extends (...args: never[]) => unknown
  ? T
  : T extends ReadonlyMap<infer K, infer V>
    ? ReadonlyMap<DeepReadonly<K>, DeepReadonly<V>>
    : T extends ReadonlySet<infer V>
      ? ReadonlySet<DeepReadonly<V>>
      : T extends readonly (infer V)[]
        ? readonly DeepReadonly<V>[]
        : T extends object
          ? { readonly [K in keyof T]: DeepReadonly<T[K]> }
          : T;

export function freezePublished<T extends object>(value: T): Readonly<T> {
  return Object.freeze(value);
}

function freezeDeep(value: unknown): unknown {
  if (value === null || typeof value !== 'object' || Object.isFrozen(value)) {
    return value;
  }

  if (Array.isArray(value)) {
    for (const item of value) freezeDeep(item);
  } else if (value instanceof Map) {
    for (const [key, item] of value) {
      freezeDeep(key);
      freezeDeep(item);
    }
  } else if (value instanceof Set) {
    for (const item of value) freezeDeep(item);
  } else {
    for (const item of Object.values(value)) freezeDeep(item);
  }

  return Object.freeze(value);
}

/** Clone once at publication time, then expose a recursively frozen snapshot. */
export function freezeProjectionSnapshot<T>(value: T): DeepReadonly<T> {
  return freezeDeep(structuredClone(value)) as DeepReadonly<T>;
}
