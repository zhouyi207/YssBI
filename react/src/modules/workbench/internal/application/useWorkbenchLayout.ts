import { useEffect, useMemo } from "react";
import { currentAppWindow } from "@/services/platform/appWindow";
import type { LayoutModelBinding } from "../layout/layoutModelBinding";
import { workbenchLayoutController } from "./workbenchLayoutController";

export function useWorkbenchLayout(binding: LayoutModelBinding): void {
  const windowLabel = useMemo(() => currentAppWindow().label, []);
  useEffect(() => {
    workbenchLayoutController.bind(binding, windowLabel);
    return () => workbenchLayoutController.unbind(binding);
  }, [binding, windowLabel]);
}
