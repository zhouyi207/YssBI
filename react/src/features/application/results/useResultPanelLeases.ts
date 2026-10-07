import { useEffect } from "react";
import { workbenchLayoutRead } from "@/modules/workbench/public";
import { resultLeases } from "./resultLeases";

/** FlexLayout owns panel existence; rendering and visibility do not own backend leases. */
export function useResultPanelLeases() {
  useEffect(() => {
    const unbind = resultLeases.bind(() => {
      if (!workbenchLayoutRead.isReady || !workbenchLayoutRead.isHydrated) return null;
      return workbenchLayoutRead
        .listPanels()
        .flatMap((panel) => (panel.metadata.role === "result" ? [panel.metadata.leaseId] : []));
    });
    const unsubscribe = workbenchLayoutRead.subscribePanelSet(resultLeases.reconcile);
    return () => {
      unsubscribe();
      unbind();
    };
  }, []);
}
