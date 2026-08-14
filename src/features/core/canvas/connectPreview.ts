import { useGraphInteractionStore, getCanvasInteraction } from '@/features/core/graphInteraction/graphInteractionStore';
import type { Pin } from '@/shared/types/domain';
import type { ConnectionFeedback } from './connectionInteraction';

export interface CanvasPreviewScope { graphPath: string; groupId: string }
export type ConnectPreviewState = {
  active: boolean;
  startPin: Pin | null;
  worldX: number;
  worldY: number;
  groupId?: string;
  feedback: ConnectionFeedback | null;
  highlightedConnectionIds: string[];
};
const IDLE: ConnectPreviewState = { active: false, startPin: null, worldX: 0, worldY: 0, feedback: null, highlightedConnectionIds: [] };
const cache = new Map<string, { interactions: object; preview: ConnectPreviewState }>();

export function getConnectPreview(scope: CanvasPreviewScope): ConnectPreviewState {
  const interactions = useGraphInteractionStore.getState().interactions;
  const key = `${scope.graphPath}\u0000${scope.groupId}`;
  const cached = cache.get(key);
  if (cached?.interactions === interactions) return cached.preview;
  const interaction = getCanvasInteraction(useGraphInteractionStore.getState(), scope.graphPath, scope.groupId);
  if (interaction.type !== 'drawingConnection' && interaction.type !== 'movingConnections') {
    cache.set(key, { interactions, preview: IDLE });
    return IDLE;
  }
  const feedback = interaction.session.feedback;
  const end = interaction.session.snappedWorld ?? { x: interaction.session.worldX, y: interaction.session.worldY };
  const preview: ConnectPreviewState = {
    active: true,
    startPin: interaction.session.source as Pin,
    worldX: end.x,
    worldY: end.y,
    groupId: interaction.session.groupId,
    feedback,
    highlightedConnectionIds: feedback?.kind === 'replace' ? feedback.displacedConnectionIds : [],
  };
  cache.set(key, { interactions, preview });
  return preview;
}

export function subscribeConnectPreview(listener: () => void): () => void {
  return useGraphInteractionStore.subscribe(listener);
}
