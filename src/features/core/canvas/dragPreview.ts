import { useGraphInteractionStore, getCanvasInteraction } from '@/features/core/graphInteraction/graphInteractionStore';
import type { CanvasPreviewScope } from './connectPreview';

export type DragPreviewState = {
  active: boolean;
  dragDelta: { x: number; y: number };
  dragNodeIds: ReadonlySet<string>;
  groupId?: string;
};
const IDLE: DragPreviewState = { active: false, dragDelta: { x: 0, y: 0 }, dragNodeIds: new Set<string>() };

export function getDragPreview(scope: CanvasPreviewScope): DragPreviewState {
  const interaction = getCanvasInteraction(useGraphInteractionStore.getState(), scope.graphPath, scope.groupId);
  if (interaction.type !== 'draggingNodes') return IDLE;
  return {
    active: true,
    dragDelta: interaction.session.delta,
    dragNodeIds: new Set(interaction.session.nodeIds),
    groupId: interaction.session.groupId,
  };
}

export function subscribeDragPreview(listener: () => void): () => void {
  return useGraphInteractionStore.subscribe(listener);
}
