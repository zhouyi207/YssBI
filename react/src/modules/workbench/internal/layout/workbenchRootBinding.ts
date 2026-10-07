import type { Action } from "flexlayout-react";
import { LayoutModelBinding } from "./layoutModelBinding";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { configureWorkbenchModel } from "./workbenchActivityGroup";
import { workbenchLayoutInternal } from "./workbenchLayoutInternal";

/** Native renderer entry points; business callers use the read/control contracts. */
export interface WorkbenchLayoutRootBinding {
  create(): LayoutModelBinding;
  dispatchAction(action: Action): void;
  activatePanel(id: string): void;
}
export const workbenchLayoutRootBinding: WorkbenchLayoutRootBinding = {
  create: () => new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel),
  dispatchAction: (action) => workbenchLayoutInternal.dispatchAction(action),
  activatePanel: (id) => workbenchLayoutInternal.activatePanel(id),
};
