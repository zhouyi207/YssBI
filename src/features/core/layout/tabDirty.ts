import { useLayoutStore } from "./layoutStore";

export function markGraphTabDirty(graphId: string): void {
    useLayoutStore.getState().setTabDirty(graphId, true);
}
