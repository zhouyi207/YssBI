import { useEffect } from 'react';
import {
  getSelectionSession,
  subscribeSelectionSession,
  type ActiveSelectionSession,
} from './selectionSession';

const BOX_CLASS =
  'absolute pointer-events-none z-50 border-2 border-dashed border-[var(--accent-color)] bg-[var(--selection-region)]/15';

function applySelectionBox(
  boxEl: HTMLDivElement,
  canvasEl: HTMLDivElement,
  session: ActiveSelectionSession,
): void {
  const canvasBounds = canvasEl.getBoundingClientRect();
  boxEl.style.display = 'block';
  boxEl.style.left = `${Math.min(session.startX, session.currentX) - canvasBounds.left}px`;
  boxEl.style.top = `${Math.min(session.startY, session.currentY) - canvasBounds.top}px`;
  boxEl.style.width = `${Math.abs(session.startX - session.currentX)}px`;
  boxEl.style.height = `${Math.abs(session.startY - session.currentY)}px`;
}

/** Imperative marquee rect — no React re-render per pointer frame. */
export function useSelectionBoxPreview(
  boxRef: React.RefObject<HTMLDivElement | null>,
  canvasRef: React.RefObject<HTMLDivElement | null>,
  groupId: string | undefined,
): void {
  useEffect(() => {
    const box = boxRef.current;
    const canvas = canvasRef.current;
    if (!box || !canvas || !groupId) return;

    if (!box.className) box.className = BOX_CLASS;

    const sync = () => {
      const session = getSelectionSession();
      if (!session.active || session.groupId !== groupId) {
        box.style.display = 'none';
        return;
      }
      applySelectionBox(box, canvas, session);
    };

    return subscribeSelectionSession(sync);
  }, [boxRef, canvasRef, groupId]);
}
