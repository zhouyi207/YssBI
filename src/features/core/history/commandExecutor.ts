/**
 * Command Executor — Central pipeline for execute / undo / redo.
 *
 * executeCommand() is the single entry point for all editor mutations
 * that should participate in undo/redo.
 */

import { useHistoryStore } from './historyStore';
import { getCommandHandler } from './commands';
import { notifyStructuralChange } from './structuralChange';
import type { CommandType, ExecuteOptions } from './types';

/**
 * Execute an editor command and push it onto the undo stack.
 *
 * @param graphPath - Target graph
 * @param type    - Registered command type
 * @param args    - Command-specific arguments
 * @param options - mergeKey for operation coalescing
 */
export async function executeCommand<TArgs = unknown>(
  graphPath: string,
  type: CommandType,
  args: TArgs,
  options?: ExecuteOptions,
): Promise<unknown> {
  const handler = getCommandHandler(type);
  const context = await handler.execute(graphPath, args);
  useHistoryStore.getState().push(graphPath, type, context, options);
  notifyStructuralChange(type, graphPath);
  return context;
}
