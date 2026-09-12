import type { WindowKind } from "./createPersistedWindow";

/** Map SPA hash route to persisted window kind. */
export function windowKindForRoute(route: string): WindowKind {
  switch (route) {
    case "/plot":
      return "plot";
    case "/inspect":
      return "inspect";
    case "/info":
      return "info";
    default:
      return "info";
  }
}
