/**
 * Graph resource path helpers.
 *
 * Keep frontend key/path handling aligned with Rust:
 * - normalize: slash normalization + compact separators
 * - encode: use "::" for resource-key-safe IDs
 * - decode: restore "/" and re-normalize
 *
 * Persisted graphs: `events/...` / `functions/...` relative paths.
 *
 * `GraphResourceUri` is the logical URI form (`yssbi://graph/{kind}/{encodedPath}`).
 * `Graph.path` / `LayoutTab.id` / `ResourceRef.id` use the persisted relative path.
 */

export type GraphResourceKind = 'event' | 'function';

/** Logical URI for persisted graph resources. */
export type GraphResourceUri = `yssbi://graph/${GraphResourceKind}/${string}`;

const GRAPH_URI_PREFIX = 'yssbi://graph/';

export function normalizeGraphResourcePath(path: string): string {
  return path
    .replace(/\\/g, '/')
    .replace(/^\/+|\/+$/g, '')
    .split('/')
    .filter((part) => part.length > 0)
    .join('/');
}

export function encodeGraphResourceKey(path: string): string {
  return normalizeGraphResourcePath(path).replace(/\//g, '::');
}

export function decodeGraphResourceKey(encoded: string): string {
  return normalizeGraphResourcePath(encoded.replace(/::/g, '/'));
}

/** Infer graph kind from a persisted path. */
export function inferGraphResourceKind(path: string): GraphResourceKind | undefined {
  const normalized = normalizeGraphResourcePath(path);
  if (normalized.startsWith('events/')) return 'event';
  if (normalized.startsWith('functions/')) return 'function';
  return undefined;
}

/** Tab / ResourceRef id for graph resources: persisted relative path. */
export function isValidGraphResourceTabId(path: string, kind: GraphResourceKind): boolean {
  const inferred = inferGraphResourceKind(path);
  return inferred === kind;
}

export function toGraphResourceUri(kind: GraphResourceKind, path: string): GraphResourceUri {
  return `${GRAPH_URI_PREFIX}${kind}/${encodeGraphResourceKey(path)}`;
}

export function parseGraphResourceUri(
  uri: string,
): { kind: GraphResourceKind; path: string } | null {
  if (!uri.startsWith(GRAPH_URI_PREFIX)) return null;
  const rest = uri.slice(GRAPH_URI_PREFIX.length);
  const slash = rest.indexOf('/');
  if (slash <= 0) return null;
  const kind = rest.slice(0, slash);
  if (kind !== 'event' && kind !== 'function') return null;
  const encoded = rest.slice(slash + 1);
  if (!encoded) return null;
  return { kind, path: decodeGraphResourceKey(encoded) };
}

/** Map a persisted graph resource path to its logical URI (kind required). */
export function graphResourceUriFromPath(kind: GraphResourceKind, path: string): GraphResourceUri {
  return toGraphResourceUri(kind, normalizeGraphResourcePath(path));
}

/** Extract graph path from URI; returns null if not a graph resource URI. */
export function graphPathFromResourceUri(uri: string): string | null {
  return parseGraphResourceUri(uri)?.path ?? null;
}
