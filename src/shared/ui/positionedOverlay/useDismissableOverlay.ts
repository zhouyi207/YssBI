import { useEffect, type RefObject } from 'react';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

export interface UseDismissableOverlayOptions {
  ref: RefObject<HTMLElement | null>;
  onDismiss?: () => void;
  enabled?: boolean;
}

/** Shared outside-pointer and Escape behavior for custom positioned overlays. */
export function useDismissableOverlay({
  ref,
  onDismiss,
  enabled = true,
}: UseDismissableOverlayOptions): void {
  useEffect(() => {
    if (!enabled || !onDismiss) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && ref.current?.contains(target)) return;
      onDismiss();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      onDismiss();
    };

    const cleanupPointerDown = addGlobalEventListener(
      window,
      'pointerdown',
      handlePointerDown,
      { capture: true },
    );
    const cleanupKeyDown = addGlobalEventListener(window, 'keydown', handleKeyDown);
    return () => {
      cleanupPointerDown();
      cleanupKeyDown();
    };
  }, [enabled, onDismiss, ref]);
}
