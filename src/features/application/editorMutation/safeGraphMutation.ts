
import { executeCommandOutcome } from '@/features/core/history';
import type { GraphMutationCommandInvocation } from '@/features/core/history/commandExecutor';
import type { GraphMutationCommandResult } from '@/features/core/history/types';
import { logger } from '@/utils/appLogger';


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
