import { useEffect } from 'react';
import {
  useGraphInteractionStore,
  type GraphInteractionState,
  type SelectionSession,
} from '@/features/core/graphInteraction/graphInteractionStore';

const BOX_CLASS =
  'absolute pointer-events-none z-50 border-2 border-dashed border-[var(--accent-color)] bg-[var(--selection-region)]/15';

export function selectionInteractionForScope(
  state: Pick<GraphInteractionState, 'interactions'>,
  graphPath: string,
  groupId: string,
): { type: 'selecting'; session: SelectionSession } | null {
  const interaction = state.interactions[graphPath];
  return interaction?.type === 'selecting' && interaction.session.groupId === groupId
    ? interaction
    : null;
}

export function useSelectionBoxPreview(
  boxRef: React.RefObject<HTMLDivElement | null>,
  canvasElementRef: React.RefObject<HTMLDivElement | null>,
  graphPath: string | undefined,
  groupId: string | undefined,
): void {
  useEffect(() => {
    const box = boxRef.current;
    const canvas = canvasElementRef.current;
    if (!box || !canvas || !graphPath || !groupId) return;
    if (!box.className) box.className = BOX_CLASS;
    const sync = () => {
      const interaction = selectionInteractionForScope(
        useGraphInteractionStore.getState(),
        graphPath,
        groupId,
      );
      if (!interaction || interaction.type !== 'selecting') {
        box.style.display = 'none';
        return;
      }
      const bounds = canvas.getBoundingClientRect();
      const session = interaction.session;
      box.style.display = 'block';
      box.style.left = `${Math.min(session.startX, session.currentX) - bounds.left}px`;
      box.style.top = `${Math.min(session.startY, session.currentY) - bounds.top}px`;
      box.style.width = `${Math.abs(session.startX - session.currentX)}px`;
      box.style.height = `${Math.abs(session.startY - session.currentY)}px`;
    };
    sync();
    return useGraphInteractionStore.subscribe(sync);
  }, [boxRef, canvasElementRef, graphPath, groupId]);
}
