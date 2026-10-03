type DragEventInput = {
  activatorEvent: Event | null;
  delta?: { x: number; y: number };
};

function hasClientPoint(
  event: Event | null,
): event is Event & { clientX: number; clientY: number } {
  return Boolean(event && typeof event === "object" && "clientX" in event && "clientY" in event);
}

export function resolveDragClientPoint(event: DragEventInput): { x: number; y: number } | null {
  const activator = event.activatorEvent;
  if (!hasClientPoint(activator)) {
    return null;
  }
  const delta = event.delta ?? { x: 0, y: 0 };
  return {
    x: activator.clientX + delta.x,
    y: activator.clientY + delta.y,
  };
}
