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

export function subscribeDragPreview(
  scope: CanvasPreviewScope,
  listener: () => void,
): () => void {
  const initialState = useGraphInteractionStore.getState();
  let previousInteraction = getCanvasInteraction(
    initialState,
    scope.graphPath,
    scope.groupId,
  );
  let previousOverrides = initialState.positionOverrides[scope.graphPath];
  return useGraphInteractionStore.subscribe((state) => {
    const nextInteraction = getCanvasInteraction(state, scope.graphPath, scope.groupId);
    const nextOverrides = state.positionOverrides[scope.graphPath];
    const interactionChanged = nextInteraction !== previousInteraction;
    const settledOverridesChanged = nextInteraction.type !== 'draggingNodes'
      && nextOverrides !== previousOverrides;
    previousInteraction = nextInteraction;
    previousOverrides = nextOverrides;
    if (interactionChanged || settledOverridesChanged) listener();
  });
}
