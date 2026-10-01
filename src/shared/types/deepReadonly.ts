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

// Memoize containers and explicit publication roots. Recording every scalar-only leaf
// and empty array would grow the weak table without avoiding any recursive traversal.
const publishedObjects = new WeakSet<object>();
let freezeDepth = 0;

/** A memoized deep-freeze proof for owned DTOs; shallow Object.freeze is insufficient. */
export function isPublishedValue(value: unknown): value is object {
  return (
    value !== null && typeof value === "object" && freezeDepth === 0 && publishedObjects.has(value)
  );
}

function freezeDeep(value: unknown): unknown {
  if (value === null || typeof value !== "object" || publishedObjects.has(value)) {
    return value;
  }
  // The visited set also breaks cycles. Do not certify immutable DTOs mid-traversal.
  freezeDepth++;
  try {
    if (Array.isArray(value)) {
      if (value.length > 0) publishedObjects.add(value);
      for (let index = 0; index < value.length; index++) freezeDeep(value[index]);
    } else if (value instanceof Map) {
      publishedObjects.add(value);
      for (const [key, item] of value) {
        freezeDeep(key);
        freezeDeep(item);
      }
    } else if (value instanceof Set) {
      publishedObjects.add(value);
      for (const item of value) freezeDeep(item);
    } else {
      // Publication visits every DTO object. Enumerate in place instead of allocating a
      // temporary values array for each node, port, parameter and derived index entry.
      const record = value as Record<string, unknown>;
      let hasChildren = false;
      for (const key in record) {
        if (!Object.prototype.hasOwnProperty.call(record, key)) continue;
        const child = record[key];
        if (child === null || typeof child !== "object") continue;
        if (!hasChildren) {
          publishedObjects.add(value);
          hasChildren = true;
        }
        freezeDeep(child);
      }
    }
    Object.freeze(value);
    return value;
  } catch (error) {
    publishedObjects.delete(value);
    throw error;
  } finally {
    freezeDepth--;
  }
}

/** Clone once at publication time, then expose a recursively frozen snapshot. */
export function freezeProjectionSnapshot<T>(value: T): DeepReadonly<T> {
  return freezePublishedValue(structuredClone(value));
}

/** Publish an owned, immutably updated value without making a second data copy. */
export function freezePublishedValue<T>(value: T): DeepReadonly<T> {
  freezeDeep(value);
  if (value !== null && typeof value === "object") publishedObjects.add(value);
  return value as DeepReadonly<T>;
}
