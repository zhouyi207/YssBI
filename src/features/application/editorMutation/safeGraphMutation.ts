import { i18n } from '@/app/i18n';
import { executeCommandOutcome } from '@/features/core/history';
import type { GraphMutationCommandInvocation } from '@/features/core/history/commandExecutor';
import type { GraphMutationCommandResult } from '@/features/core/history/types';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';

import { graphMutationErrorMessageKey } from './graphMutationError';

export async function executeSafeGraphMutationOutcome(
  graphPath: string,
  operation: string,
  ...invocation: GraphMutationCommandInvocation
): Promise<GraphMutationCommandResult> {
  try {
    const outcome = await executeCommandOutcome(graphPath, ...invocation);
    if (outcome === false) return false;
    const code = outcome.status === 'rejected'
      ? outcome.code
      : outcome.status === 'conflict' ? 'graph_revision_conflict' : null;
    if (code) {
      logger.graph.warn(
        `Graph mutation outcome code=${code} graphPath=${graphPath} operation=${operation}`,
        'GraphMutation',
      );
      const key = graphMutationErrorMessageKey({ code });
      if (key) uiStore.showToast(i18n.t(key), 'error');
    }
    return outcome;
  } catch {
    logger.graph.warn(
      `Graph mutation command failed graphPath=${graphPath} operation=${operation}`,
      'GraphMutation',
    );
    return false;
  }
}

export async function executeSafeGraphMutation(
  graphPath: string,
  operation: string,
  ...invocation: GraphMutationCommandInvocation
): Promise<boolean> {
  const outcome = await executeSafeGraphMutationOutcome(graphPath, operation, ...invocation);
  return outcome !== false && outcome.status === 'applied';
}
