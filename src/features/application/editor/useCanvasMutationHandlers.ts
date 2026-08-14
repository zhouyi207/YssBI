import { useMemo } from 'react';
import { i18n } from '@/app/i18n';
import type {
  CanvasInteractionHandlers,
  CanvasMutationOutcome,
} from '@/features/core/canvas';
import { executeSafeGraphMutationOutcome } from '@/features/application/editorMutation/safeGraphMutation';
import { graphMutationErrorMessageKey } from '@/features/application/editorMutation/graphMutationError';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
import { insertRerouteAtConnection } from './edgeOperations';
import { ensureGraphMutationPortRegistered } from '@/features/application/editorMutation/registerGraphMutationPort';

function toCanvasMutationOutcome(outcome: Awaited<ReturnType<typeof executeSafeGraphMutationOutcome>>): CanvasMutationOutcome {
  if (outcome !== false && outcome.status === 'applied') return { status: 'applied' };
  if (outcome === false) return { status: 'failed' };
  const code = outcome.status === 'rejected'
    ? outcome.code
    : outcome.status === 'conflict' ? 'graph_revision_conflict' : null;
  const key = code ? graphMutationErrorMessageKey({ code }) : null;
  return key ? { status: 'failed', message: i18n.t(key) } : { status: 'failed' };
}

export function createCanvasMutationHandlers(): CanvasInteractionHandlers {
  ensureGraphMutationPortRegistered();
  return {
    async submitConnection({ graphPath, intent, sourcePinId, targetPinId }) {
      const outcome = intent === 'connect'
        ? await executeSafeGraphMutationOutcome(
            graphPath,
            'Canvas connect',
            'ConnectPins',
            { pinA: sourcePinId, pinB: targetPinId },
          )
        : await executeSafeGraphMutationOutcome(
            graphPath,
            'Canvas move connections',
            'MoveConnections',
            { sourcePinId, targetPinId },
          );
      return toCanvasMutationOutcome(outcome);
    },
    async disconnectPort(graphPath, pinId) {
      return toCanvasMutationOutcome(await executeSafeGraphMutationOutcome(
        graphPath,
        'Alt disconnect port',
        'DisconnectPort',
        { pinId },
      ));
    },
    async insertRerouteAtConnection({ graphPath, connectionId, position }) {
      return toCanvasMutationOutcome(await insertRerouteAtConnection(graphPath, connectionId, position));
    },
    reportMutationFailure({ graphPath, intent, message }) {
      logger.graph.warn(
        `Graph mutation failed graphPath=${graphPath} intent=${intent}`,
        'CanvasInteraction',
      );
      uiStore.showToast(message, 'error');
    },
  };
}

export function useCanvasMutationHandlers(): CanvasInteractionHandlers {
  return useMemo(createCanvasMutationHandlers, []);
}
