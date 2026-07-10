export type {
  StatusBarAlignment,
  StatusBarItemRegistration,
  StatusBarItemViewModel,
  StatusBarItemsSnapshot,
  StatusBarRenderContext,
} from "./statusBarItemTypes";
export {
  registerStatusBarItem,
  getRegisteredStatusBarItems,
  clearStatusBarRegistryForTests,
} from "./statusBarRegistry";
export { createBuiltInStatusBarItems } from "./builtInStatusBarItems";
export type { BuiltInStatusBarActions } from "./builtInStatusBarItems";
export { buildStatusBarItems, useStatusBarSnapshot } from "./buildStatusBarItems";
