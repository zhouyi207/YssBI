export const PIN_CONNECTION_ANCHOR_SELECTOR = "[data-pin-connection-anchor]";

export interface PinConnectionAnchorMeasurement {
  pinId: string;
  center: { x: number; y: number };
}

/** Measure the explicit icon anchor for a rendered pin without falling back to its label row. */
export function measurePinConnectionAnchor(
  pinElement: HTMLElement,
): PinConnectionAnchorMeasurement | null {
  const pinId = pinElement.dataset.pinId;
  const anchor = pinElement.querySelector<HTMLElement>(PIN_CONNECTION_ANCHOR_SELECTOR);
  if (!pinId || !anchor || anchor.dataset.pinConnectionAnchor !== pinId) return null;

  const rect = anchor.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;

  return {
    pinId,
    center: {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    },
  };
}
