/** Dedicated overlay root from index.html (#portal, z-index 9999). */
export function getOverlayPortalRoot(): HTMLElement {
  return document.getElementById("portal") ?? document.body;
}
