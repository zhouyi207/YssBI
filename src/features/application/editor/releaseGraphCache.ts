import { useGraphDataStore, useVariableStore } from "@/features/core/dataStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { GraphService } from "@/services/graph/graphService";
import { logger } from "@/utils/appLogger";

function isGraphOpenInAnyTab(graphId: string): boolean {
  return Object.values(useLayoutStore.getState().nodes).some((node) =>
    node.data?.tabs?.some((tab) => tab.id === graphId)
  );
}

export function releaseGraphCacheIfClosed(graphId: string): void {
  if (isGraphOpenInAnyTab(graphId)) return;
  useGraphDataStore.getState().clearGraph(graphId);
  useVariableStore.getState().clearGraphVariables(graphId);
  void GraphService.unloadProjectGraph(graphId).catch((error) => {
    logger.graph.warn(
      `Failed to unload graph '${graphId}': ${error instanceof Error ? error.message : String(error)}`,
      "releaseGraphCache"
    );
  });
}
