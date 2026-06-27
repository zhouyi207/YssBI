export interface StructTypeMeta {
  key: string;
  parents: string[];
  category?: string;
  displayName?: string;
}

export interface TypeSystemSnapshot {
  structTypes: Record<string, StructTypeMeta>;
}

export const EMPTY_TYPE_SYSTEM: TypeSystemSnapshot = {
  structTypes: {},
};

let activeTypeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM;

export function setActiveTypeSystem(snapshot: TypeSystemSnapshot): void {
  activeTypeSystem = snapshot;
}

export function getActiveTypeSystem(): TypeSystemSnapshot {
  return activeTypeSystem;
}

export function structTypeExtends(
  sourceKey: string,
  targetKey: string,
  snapshot: TypeSystemSnapshot = activeTypeSystem,
): boolean {
  const visited = new Set<string>();
  const stack = [sourceKey];

  while (stack.length > 0) {
    const key = stack.pop()!;
    if (visited.has(key)) continue;
    visited.add(key);

    const meta = snapshot.structTypes[key];
    if (!meta) continue;

    for (const parent of meta.parents ?? []) {
      if (parent === targetKey) return true;
      stack.push(parent);
    }
  }

  return false;
}

export function structCanAccept(
  targetKey: string,
  sourceKey: string,
  snapshot: TypeSystemSnapshot = activeTypeSystem,
): boolean {
  return targetKey === sourceKey || structTypeExtends(sourceKey, targetKey, snapshot);
}
