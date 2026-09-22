import { workbenchLayoutRuntime } from "./workbenchLayoutInternal";
import type { WorkbenchLayoutControlContract } from "./workbenchTypes";

export type WorkbenchLayoutControl = WorkbenchLayoutControlContract;

export const workbenchLayoutControl: WorkbenchLayoutControl = workbenchLayoutRuntime.control;
