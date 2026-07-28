import { getCommandHandler } from './commands';
import type { AvailableCommandType, CommandHandlerMap } from './commands/registryTypes';
import { notifyStructuralChange } from './structuralChange';
import type { CommandHandler } from './types';

function isAppliedResult(result: unknown): boolean {
  if (result === true) return true;
  return typeof result === 'object'
    && result !== null
    && 'status' in result
    && (result as { status?: unknown }).status === 'applied';
}

type CommandArgs<K extends AvailableCommandType> = Parameters<CommandHandlerMap[K]['execute']>[1];

export async function executeCommand<K extends AvailableCommandType>(
  graphPath: string,
  type: K,
  args: CommandArgs<K>,
): Promise<boolean> {
  try {
    const handler = getCommandHandler(type) as CommandHandler<CommandArgs<K>>;
    const result = await handler.execute(graphPath, args);
    const applied = isAppliedResult(result);
    if (applied) notifyStructuralChange(type, graphPath);
    return applied;
  } catch {
    return false;
  }
}
