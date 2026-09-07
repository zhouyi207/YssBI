import type { DockviewApi } from "dockview-react";

import { WORKBENCH_BOTTOM_DEFAULT_ORDER } from "../dockview/workbenchDockviewDefaults";
import { readMetadata } from "../dockview/workbenchDockviewOperations";

/** Derive chrome placement from the live Dockview, including restored and resized groups. */
export function bindWorkbenchStatusBarLayout(api: DockviewApi, host: HTMLElement): () => void {
  const workbench = host.closest<HTMLElement>("[data-yssbi-workbench]");
  let frame: number | undefined;
  let observedColumn: HTMLElement | null = null;

  const update = () => {
    frame = undefined;
    const bottom = api.groups.find(
      (group) => group.api.location.type === "edge" && group.api.location.position === "bottom",
    );
    if (bottom) {
      const hidden = bottom.panels.every((panel) => {
        const metadata = readMetadata(panel);
        return (
          metadata?.role === "view" &&
          WORKBENCH_BOTTOM_DEFAULT_ORDER.some((viewId) => viewId === metadata.viewId)
        );
      });
      if (bottom.header.hidden !== hidden) {
        bottom.header.hidden = hidden;
        bottom.relayout();
      }
    }

    const middleColumn = host.querySelector<HTMLElement>(".dv-shell-middle-column");
    if (middleColumn !== observedColumn) {
      if (observedColumn) observer.unobserve(observedColumn);
      if (middleColumn) observer.observe(middleColumn);
      observedColumn = middleColumn;
    }
    if (workbench) {
      const workbenchBounds = workbench.getBoundingClientRect();
      const columnBounds = middleColumn?.getBoundingClientRect();
      const left = columnBounds ? Math.max(0, columnBounds.left - workbenchBounds.left) : 0;
      const right = columnBounds ? Math.max(0, workbenchBounds.right - columnBounds.right) : 0;
      workbench.style.setProperty("--workbench-center-offset", `${left}px`);
      workbench.style.setProperty("--workbench-center-right-offset", `${right}px`);
    }
  };
  const schedule = () => {
    if (frame === undefined) frame = requestAnimationFrame(update);
  };
  const disposable = api.onDidLayoutChange(schedule);
  const observer = new ResizeObserver(schedule);
  observer.observe(host);
  schedule();

  return () => {
    disposable.dispose();
    observer.disconnect();
    if (frame !== undefined) cancelAnimationFrame(frame);
    workbench?.style.removeProperty("--workbench-center-offset");
    workbench?.style.removeProperty("--workbench-center-right-offset");
  };
}
