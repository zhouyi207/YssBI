import { useShallow } from "zustand/react/shallow";
import { resolveSectionExpanded, type SidebarSectionKey } from "./sidebarSectionState";
import { useSidebarStore } from "./sidebarStore";

/** Subscribe only to the listed section keys (avoids whole-record re-renders). */
export function useSidebarSectionExpandSnapshot<K extends SidebarSectionKey>(
  ...keys: K[]
): Record<K, boolean> {
  return useSidebarStore(
    useShallow((state) => {
      const snapshot = {} as Record<K, boolean>;
      for (const key of keys) {
        snapshot[key] = resolveSectionExpanded(state.expandedSections, key);
      }
      return snapshot;
    }),
  );
}
