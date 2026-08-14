import { SASH_DRAG_BODY_CLASS, SASH_DRAG_END_EVENT } from '@/views/EditorView/Renderer/sashResizeLogic';

/** Skip layout work during sash drag and flush once when dragging ends. */
export function createSashAwareResizeHandler(onResize: () => void): {
  handler: () => void;
  flushAfterSashDrag: () => void;
} {
  let skippedDuringSashDrag = false;

  const handler = () => {
    if (document.body.classList.contains(SASH_DRAG_BODY_CLASS)) {
      skippedDuringSashDrag = true;
      return;
    }
    onResize();
  };

  const flushAfterSashDrag = () => {
    if (!skippedDuringSashDrag) return;
    skippedDuringSashDrag = false;
    onResize();
  };

  return { handler, flushAfterSashDrag };
}

export function bindSashAwareResizeObserver(
  target: Element,
  onResize: () => void,
): () => void {
  const { handler, flushAfterSashDrag } = createSashAwareResizeHandler(onResize);
  const observer = new ResizeObserver(handler);
  observer.observe(target);
  window.addEventListener(SASH_DRAG_END_EVENT, flushAfterSashDrag);

  return () => {
    observer.disconnect();
    window.removeEventListener(SASH_DRAG_END_EVENT, flushAfterSashDrag);
  };
}
