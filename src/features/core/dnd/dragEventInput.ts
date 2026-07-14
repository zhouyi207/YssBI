import { useModifierKeyStore } from '@/features/core/keyboard';

type DragEventInput = {
  activatorEvent: Event | null;
  delta?: { x: number; y: number };
};

export type DragModifiers = {
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
};

function hasClientPoint(
  event: Event | null,
): event is Event & { clientX: number; clientY: number } {
  return Boolean(
    event
    && typeof event === 'object'
    && 'clientX' in event
    && 'clientY' in event
  );
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

export function readDragModifiers(event: Pick<DragEventInput, 'activatorEvent'>): DragModifiers {
  const modifierStore = useModifierKeyStore.getState();
  const activator = event.activatorEvent;
  if (hasClientPoint(activator)) {
    return {
      altKey: ('altKey' in activator && Boolean(activator.altKey)) || modifierStore.altKey,
      ctrlKey: ('ctrlKey' in activator && Boolean(activator.ctrlKey)) || modifierStore.ctrlKey,
      shiftKey: ('shiftKey' in activator && Boolean(activator.shiftKey)) || modifierStore.shiftKey,
    };
  }
  return {
    altKey: modifierStore.altKey,
    ctrlKey: modifierStore.ctrlKey,
    shiftKey: modifierStore.shiftKey,
  };
}
