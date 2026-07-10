import type { StatusBarItemRegistration } from "./statusBarItemTypes";

const registry = new Map<string, StatusBarItemRegistration>();

/** Register an extension status bar item. Returns an unregister function. */
export function registerStatusBarItem(item: StatusBarItemRegistration): () => void {
  if (registry.has(item.id)) {
    console.warn(`[statusBarRegistry] Overwriting status bar item "${item.id}"`);
  }
  registry.set(item.id, item);
  return () => {
    registry.delete(item.id);
  };
}

export function getRegisteredStatusBarItems(): StatusBarItemRegistration[] {
  return Array.from(registry.values());
}

export function clearStatusBarRegistryForTests(): void {
  registry.clear();
}
