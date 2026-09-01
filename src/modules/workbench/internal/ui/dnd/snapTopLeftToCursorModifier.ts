import type { Modifier } from "@dnd-kit/core";

function readActivatorCoordinates(event: Event): { x: number; y: number } | null {
  if ("clientX" in event && "clientY" in event) {
    const { clientX, clientY } = event as { clientX: number; clientY: number };
    return { x: clientX, y: clientY };
  }
  return null;
}

/** Align DragOverlay top-left with the pointer instead of preserving grab offset. */
export const snapTopLeftToCursor: Modifier = ({ activatorEvent, draggingNodeRect, transform }) => {
  if (!draggingNodeRect || !activatorEvent) {
    return transform;
  }

  const coords = readActivatorCoordinates(activatorEvent);
  if (!coords) {
    return transform;
  }

  const offsetX = coords.x - draggingNodeRect.left;
  const offsetY = coords.y - draggingNodeRect.top;

  return {
    ...transform,
    x: transform.x + offsetX,
    y: transform.y + offsetY,
  };
};
