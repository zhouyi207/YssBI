/**
 * Echo suppressor for self-originated, idempotent backend commands.
 *
 * The frontend updates its store optimistically when issuing a command (e.g.
 * during a drag we apply positions locally before invoking the backend). The
 * backend then mutates its own state and emits an event that the frontend
 * picks up and *re-applies* to its store. For idempotent commands this
 * round-trip is wasteful and, when several commands overlap, can briefly
 * revert the UI to a stale value before catching up.
 *
 * This helper lets a command mark a key as "pending" before invoking the
 * backend; event handlers can then skip re-applying state for keys that are
 * still pending. Counts are reference-counted so overlapping operations on the
 * same key remain consistent.
 */

const pendingByDomain = new Map<string, Map<string, number>>();

function bucket(domain: string): Map<string, number> {
  let map = pendingByDomain.get(domain);
  if (!map) {
    map = new Map();
    pendingByDomain.set(domain, map);
  }
  return map;
}

/** Mark `key` as having an in-flight self-originated command in `domain`. */
export function markPending(domain: string, key: string): void {
  const map = bucket(domain);
  map.set(key, (map.get(key) ?? 0) + 1);
}

/** Mark a previously-pending `key` as resolved. Idempotent for unknown keys. */
export function resolvePending(domain: string, key: string): void {
  const map = bucket(domain);
  const next = (map.get(key) ?? 0) - 1;
  if (next <= 0) map.delete(key);
  else map.set(key, next);
}

/** Whether `key` currently has any in-flight self-originated commands. */
export function isPending(domain: string, key: string): boolean {
  return (pendingByDomain.get(domain)?.get(key) ?? 0) > 0;
}

/** Convenience: wrap a promise to auto-resolve pending markers on settle. */
export function trackPending<T>(domain: string, keys: readonly string[], task: Promise<T>): Promise<T> {
  for (const key of keys) markPending(domain, key);
  return task.finally(() => {
    for (const key of keys) resolvePending(domain, key);
  });
}
