import { isGraphResourceDirty } from '@/features/core/resource';
import { useLayoutStore } from './layoutStore';

export function isGraphOpenInAnyTab(graphPath: string): boolean {
  return Object.values(useLayoutStore.getState().nodes).some((node) =>
    node.data?.tabs?.some((tab) => tab.id === graphPath),
  );
}

export function isGraphTabDirty(graphPath: string): boolean {
  return isGraphResourceDirty(graphPath);
}
