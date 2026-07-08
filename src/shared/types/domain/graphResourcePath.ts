/**
 * Graph resource path helpers.
 *
 * Keep frontend key/path handling aligned with Rust:
 * - normalize: slash normalization + compact separators
 * - encode: use "::" for resource-key-safe IDs
 * - decode: restore "/" and re-normalize
 *
 * Persisted graphs: `events/...` / `functions/...` relative paths.
 * Unsaved drafts: `untitled:{kind}:{label}` handles (tab id = handle).
 *
 * `GraphResourceUri` is the logical URI form (`yssbi://graph/{kind}/{encodedPath}`).
 * `Graph.path` / `LayoutTab.id` / `ResourceRef.id` remain path or untitled handle.
 */

export type GraphResourceKind = 'event' | 'function';

/** Logical URI for persisted graph resources. */
export type GraphResourceUri = `yssbi://graph/${GraphResourceKind}/${string}`;

/** In-memory draft handle before first save (`untitled:event:Untitled-1`). */
export type UntitledGraphPath = `untitled:${GraphResourceKind}:${string}`;

const GRAPH_URI_PREFIX = 'yssbi://graph/';
const UNTITLED_PREFIX = 'untitled:';

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

export function isUntitledGraphPath(path: string): path is UntitledGraphPath {
  return path.startsWith(UNTITLED_PREFIX);
}

export function isPersistedGraphPath(path: string): boolean {
  return !isUntitledGraphPath(path);
}

export function parseUntitledGraphPath(
  path: string,
): { kind: GraphResourceKind; label: string } | null {
  if (!isUntitledGraphPath(path)) return null;
  const rest = path.slice(UNTITLED_PREFIX.length);
  const colon = rest.indexOf(':');
  if (colon <= 0) return null;
  const kind = rest.slice(0, colon);
  if (kind !== 'event' && kind !== 'function') return null;
  const label = rest.slice(colon + 1);
  if (!label) return null;
  return { kind, label };
}

export function buildUntitledGraphPath(kind: GraphResourceKind, label: string): UntitledGraphPath {
  return `untitled:${kind}:${label}`;
}

/** Infer graph kind from a persisted path or untitled handle. */
export function inferGraphResourceKind(path: string): GraphResourceKind | undefined {
  const untitled = parseUntitledGraphPath(path);
  if (untitled) return untitled.kind;
  const normalized = normalizeGraphResourcePath(path);
  if (normalized.startsWith('events/')) return 'event';
  if (normalized.startsWith('functions/')) return 'function';
  return undefined;
}

/** Tab / ResourceRef id for graph resources: persisted path or untitled handle. */
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
