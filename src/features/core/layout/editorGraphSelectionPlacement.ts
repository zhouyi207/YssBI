export interface GraphSelectionPlacement {
  activeTabId: string | null;
  selectedNodeIds: string[];
  selectedConnectionIds?: string[];
}

function uniqueIds(ids: readonly string[] | undefined): string[] {
  return [...new Set(ids ?? [])];
}

export function replacePlacementActiveTab(
  placement: GraphSelectionPlacement,
  activeTabId: string | null,
): void {
  if (placement.activeTabId !== activeTabId) {
    placement.selectedNodeIds = [];
    placement.selectedConnectionIds = [];
  }
  placement.activeTabId = activeTabId;
}

export function remapPlacementActiveTab(
  placement: GraphSelectionPlacement,
  from: string,
  to: string,
): void {
  if (placement.activeTabId === from) placement.activeTabId = to;
}

export function normalizePlacementGraphSelection(
  placement: Pick<GraphSelectionPlacement, 'selectedNodeIds' | 'selectedConnectionIds'>,
): { selectedNodeIds: string[]; selectedConnectionIds: string[] } {
  const selectedConnectionIds = uniqueIds(placement.selectedConnectionIds);
  return selectedConnectionIds.length > 0
    ? { selectedNodeIds: [], selectedConnectionIds }
    : {
        selectedNodeIds: uniqueIds(placement.selectedNodeIds),
        selectedConnectionIds: [],
      };
}
