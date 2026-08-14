import { getCommandHandler } from './commands';
import type { AvailableCommandType, CommandHandlerMap } from './commands/registryTypes';
import { notifyStructuralChange } from './structuralChange';
import type { CommandHandler, GraphMutationCommandResult } from './types';

function isAppliedResult(result: unknown): boolean {
  if (result === true) return true;
  return typeof result === 'object'
    && result !== null
    && 'status' in result
    && (result as { status?: unknown }).status === 'applied';
}

type CommandArgs<K extends AvailableCommandType> = Parameters<CommandHandlerMap[K]['execute']>[1];
type CommandResult<K extends AvailableCommandType> =
  CommandHandlerMap[K] extends CommandHandler<CommandArgs<K>, infer TResult> ? TResult : never;

export type CommandInvocation<
  K extends AvailableCommandType = AvailableCommandType,
> = {
  [T in K]: [type: T, args: CommandArgs<T>];
}[K];

export type GraphMutationCommandType = {
  [K in AvailableCommandType]: CommandResult<K> extends GraphMutationCommandResult ? K : never;
}[AvailableCommandType];

export type GraphMutationCommandInvocation = CommandInvocation<GraphMutationCommandType>;

function executeRegisteredCommand<K extends AvailableCommandType>(
  graphPath: string,
  type: K,
  args: CommandArgs<K>,
): Promise<CommandResult<K>> | CommandResult<K> {
  const handler = getCommandHandler(type) as CommandHandler<CommandArgs<K>, CommandResult<K>>;
  return handler.execute(graphPath, args);
}

export async function executeCommandOutcome(
  graphPath: string,
  ...invocation: GraphMutationCommandInvocation
): Promise<GraphMutationCommandResult>;
export async function executeCommandOutcome(
  graphPath: string,
  ...invocation: CommandInvocation
): Promise<GraphMutationCommandResult> {
  const [type, args] = invocation;
  const result = await executeRegisteredCommand(graphPath, type, args);
  if (isAppliedResult(result)) notifyStructuralChange(type, graphPath);
  return result;
}

export async function executeCommand(
  graphPath: string,
  ...invocation: CommandInvocation
): Promise<boolean> {
  try {
    return isAppliedResult(await executeCommandOutcome(graphPath, ...invocation));
  } catch {
    return false;
  }
}
