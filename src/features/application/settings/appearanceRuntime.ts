/** Scroll area viewports only — not canvas pan/zoom or menubar chrome. */
export function applySmoothScrollSetting(enabled: boolean): void {
  document.documentElement.dataset.smoothScroll = enabled ? "true" : "false";
}
