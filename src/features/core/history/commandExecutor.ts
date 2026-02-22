/**
 * Command Executor — Central pipeline for execute / undo / redo.
 *
 * executeCommand() is the single entry point for all editor mutations
 * that should participate in undo/redo.
 */

import { useHistoryStore } from './historyStore';
import { getCommandHandler } from './commands';
import type { CommandType, ExecuteOptions } from './types';

/**
 * Execute an editor command and push it onto the undo stack.
 *
 * @param graphId - Target graph
 * @param type    - Registered command type
 * @param args    - Command-specific arguments
 * @param options - mergeKey for operation coalescing
 */
export async function executeCommand<TArgs = unknown>(
  graphId: string,
  type: CommandType,
  args: TArgs,
  options?: ExecuteOptions,
): Promise<unknown> {
  const handler = getCommandHandler(type);
  const context = await handler.execute(graphId, args);
  useHistoryStore.getState().push(graphId, type, context, options);
  return context;
}
