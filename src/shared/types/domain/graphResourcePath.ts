export type GraphResourceKind = "event" | "function";
export type GraphResourceUri = `yssbi://graph/${GraphResourceKind}/${string}`;

/** Store keys preserve the exact opaque identity supplied by Rust. */
export function toGraphResourceUri(kind: GraphResourceKind, path: string): GraphResourceUri {
  return `yssbi://graph/${kind}/${encodeURIComponent(path)}`;
}
