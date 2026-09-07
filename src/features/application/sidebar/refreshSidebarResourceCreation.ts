import { refreshProjectResourceIndex } from "@/features/application/project/projectHydration";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { useVariableStore } from "@/features/core/dataStore/variableStore";

type SidebarResourceKind = "variable" | "database";

function currentResourcePath(kind: SidebarResourceKind, id: string): string | undefined {
  return kind === "variable"
    ? useVariableStore.getState().variables[id]?.resourcePath
    : useDatabaseStore.getState().databases[id]?.resourcePath;
}

export async function refreshMissingSidebarResourcePath(options: {
  kind: SidebarResourceKind;
  id: string;
  hasCurrentDescriptor(resourcePath: string): boolean;
  refreshCatalog(): void;
}): Promise<void> {
  const refreshed = await refreshProjectResourceIndex();
  if (!refreshed) return;

  const resourcePath = currentResourcePath(options.kind, options.id);
  if (resourcePath && !options.hasCurrentDescriptor(resourcePath)) {
    options.refreshCatalog();
  }
}
