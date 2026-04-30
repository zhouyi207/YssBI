export function isAppModalOpen(): boolean {
  return document.querySelector('[data-slot="dialog-content"]') !== null;
}
