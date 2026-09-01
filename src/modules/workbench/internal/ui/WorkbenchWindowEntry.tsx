import { lazy } from "react";

export const WorkbenchWindow = lazy(async () => {
  const module = await import("./WorkbenchWindow");
  return { default: module.WorkbenchWindow };
});
