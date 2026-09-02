import { getCommandHandler } from "./commands";
import type { AvailableCommandType, CommandHandlerMap } from "./commands/registryTypes";
import { notifyStructuralChange } from "./structuralChange";
import type { CommandHandler, GraphDraftCommandResult } from "./types";

function isAppliedResult(result: unknown): boolean {
  if (result === true) return true;
  return (
    typeof result === "object" &&
    result !== null &&
    "status" in result &&
    (result as { status?: unknown }).status === "applied"
  );
}

type CommandArgs<K extends AvailableCommandType> = Parameters<CommandHandlerMap[K]["execute"]>[1];
type CommandResult<K extends AvailableCommandType> =
  CommandHandlerMap[K] extends CommandHandler<CommandArgs<K>, infer TResult> ? TResult : never;

export type CommandInvocation<K extends AvailableCommandType = AvailableCommandType> = {
  [T in K]: [type: T, args: CommandArgs<T>];
}[K];

export type GraphDraftCommandType = {
  [K in AvailableCommandType]: CommandResult<K> extends GraphDraftCommandResult ? K : never;
}[AvailableCommandType];

export type GraphDraftCommandInvocation = CommandInvocation<GraphDraftCommandType>;

function executeRegisteredCommand<K extends AvailableCommandType>(
  graphPath: string,
  type: K,
  args: CommandArgs<K>,
): Promise<CommandResult<K>> | CommandResult<K> {
  const handler = getCommandHandler(type) as CommandHandler<CommandArgs<K>, CommandResult<K>>;
  return handler.execute(graphPath, args);
}

async function executeAndNotify<K extends AvailableCommandType>(
  graphPath: string,
  type: K,
  args: CommandArgs<K>,
): Promise<CommandResult<K>> {
  const result = await executeRegisteredCommand(graphPath, type, args);
  if (isAppliedResult(result)) notifyStructuralChange(type, graphPath);
  return result;
}

export async function executeCommandWithResult<K extends AvailableCommandType>(
  graphPath: string,
  type: K,
  args: CommandArgs<K>,
): Promise<CommandResult<K> | null> {
  try {
    return await executeAndNotify(graphPath, type, args);
  } catch {
    return null;
  }
}

export async function executeCommandOutcome(
  graphPath: string,
  ...invocation: GraphDraftCommandInvocation
): Promise<GraphDraftCommandResult>;
export async function executeCommandOutcome(
  graphPath: string,
  ...invocation: CommandInvocation
): Promise<GraphDraftCommandResult> {
  const [type, args] = invocation;
  return executeAndNotify(graphPath, type, args);
}

export async function executeCommand(
  graphPath: string,
  ...invocation: CommandInvocation
): Promise<boolean> {
  const [type, args] = invocation;
  return isAppliedResult(await executeCommandWithResult(graphPath, type, args));
}
