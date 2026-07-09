/**
 * Command Executor — Central pipeline for execute / undo / redo.
 *
 * executeCommand() is the single entry point for all editor mutations
 * that should participate in undo/redo.
 */

import { useHistoryStore } from './historyStore';
import { getCommandHandler } from './commands';
import type { CommandArgsByType, CommandContextByType } from './commands/registryTypes';
import { notifyStructuralChange } from './structuralChange';
import type { CommandType, ExecuteOptions } from './types';

/**
 * Execute an editor command and push it onto the undo stack.
 */
export async function executeCommand<K extends CommandType>(
  graphPath: string,
  type: K,
  args: CommandArgsByType[K],
  options?: ExecuteOptions,
): Promise<CommandContextByType[K]> {
  const handler = getCommandHandler(type);
  const context = await handler.execute(graphPath, args);
  useHistoryStore.getState().push(graphPath, type, context, options);
  notifyStructuralChange(type, graphPath);
  return context;
}
