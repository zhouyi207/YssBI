export type CanvasShortcut =
  | "cancel"
  | "selectAll"
  | "focusSelection"
  | "fitAll"
  | "deleteSelection";

export function resolveCanvasShortcut(
  event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey" | "repeat">,
): CanvasShortcut | null {
  if (event.key === "Escape") return "cancel";
  if (event.key === "Delete" || event.key === "Backspace") return "deleteSelection";
  if (event.repeat) return null;
  const control = event.ctrlKey || event.metaKey;
  if (control && event.key.toLowerCase() === "a") return "selectAll";
  if (control || event.altKey || event.shiftKey) return null;
  if (event.key.toLowerCase() === "f") return "focusSelection";
  if (event.key === "Home") return "fitAll";
  return null;
}
