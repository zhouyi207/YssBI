import { useEffect } from "react";
import { workbenchDockviewRead } from "@/modules/workbench/public";
import { resultLeases } from "./resultLeases";

/** Dockview owns panel existence; rendering and visibility do not own backend leases. */
export function useResultPanelLeases() {
  useEffect(() => {
    const unbind = resultLeases.bind(() => {
      if (!workbenchDockviewRead.isReady || !workbenchDockviewRead.isHydrated) return null;
      return workbenchDockviewRead
        .listPanels()
        .flatMap((panel) => (panel.metadata.role === "result" ? [panel.metadata.leaseId] : []));
    });
    const unsubscribe = workbenchDockviewRead.subscribe(resultLeases.reconcile);
    return () => {
      unsubscribe();
      unbind();
    };
  }, []);
}
