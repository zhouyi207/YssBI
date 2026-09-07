import { refreshProjectResourceIndex } from "@/features/application/project/projectHydration";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";

export async function refreshMissingSidebarResourcePath(options: {
  id: string;
  hasCurrentDescriptor(resourcePath: string): boolean;
  refreshCatalog(): void;
}): Promise<void> {
  const refreshed = await refreshProjectResourceIndex();
  if (!refreshed) return;

  const resourcePath = useDatabaseStore.getState().databases[options.id]?.resourcePath;
  if (resourcePath && !options.hasCurrentDescriptor(resourcePath)) {
    options.refreshCatalog();
  }
}
