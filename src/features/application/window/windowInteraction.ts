import { TAURI_NO_DRAG_STYLE, stopTauriDragPropagation } from "@/shared/platform/tauriWebview";

/** Platform-neutral props for interactive controls placed inside a drag region. */
export const windowInteractiveRegionProps = {
  style: TAURI_NO_DRAG_STYLE,
  onPointerDown: stopTauriDragPropagation,
};
