import { useGraphDataStore } from '@/features/core/dataStore';

/** Rust projection is authoritative for whether a concrete port instance can be removed. */
export function useRepeatablePinRemovable(nodeId: string, pinId: string, graphPath?: string): boolean {
  return useGraphDataStore((state) => {
    if (!graphPath) return false;
    const pin = state.getGraphPin(graphPath, pinId);
    return pin?.nodeId === nodeId && pin.canRemove === true;
  });
}
