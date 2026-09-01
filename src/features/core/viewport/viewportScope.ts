/** VS Code-style pane viewport identity: shared graph document, independent pan/zoom per editor group. */
export interface ViewportScope {
  groupId: string;
  graphPath: string;
}

const SCOPE_SEP = "\x1e";

export function editorViewportScope(groupId: string, graphPath: string): ViewportScope {
  return { groupId, graphPath };
}

export function viewportScopeKey(scope: ViewportScope): string {
  return `${scope.groupId}${SCOPE_SEP}${scope.graphPath}`;
}

export function parseViewportScopeKey(key: string): ViewportScope | null {
  const sep = key.indexOf(SCOPE_SEP);
  if (sep <= 0 || sep >= key.length - 1) return null;
  return {
    groupId: key.slice(0, sep),
    graphPath: key.slice(sep + 1),
  };
}

export function scopeMatchesGraphPath(scope: ViewportScope, graphPath: string): boolean {
  return scope.graphPath === graphPath;
}
