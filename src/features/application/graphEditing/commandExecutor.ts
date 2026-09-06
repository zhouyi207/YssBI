import { commandRegistry } from "./commands";
import type { CommandHandlerMap } from "./commands/registryTypes";
import { notifyStructuralChange } from "./structuralChange";
import type { CommandHandler, GraphEditOutcome } from "./types";
import { logger } from "@/features/application/observability/appLogger";

export type GraphEditInvocation = {
  [K in keyof CommandHandlerMap]: [type: K, args: Parameters<CommandHandlerMap[K]["execute"]>[1]];
}[keyof CommandHandlerMap];

export async function executeGraphEdit(
  graphPath: string,
  ...[type, args]: GraphEditInvocation
): Promise<GraphEditOutcome> {
  try {
    const handler = commandRegistry[type] as CommandHandler<typeof args, GraphEditOutcome>;
    const outcome = await handler.execute(graphPath, args);
    if (outcome.status === "applied") notifyStructuralChange(type, graphPath);
    return outcome;
  } catch {
    logger.graph.warn(`Graph edit failed graphPath=${graphPath} command=${type}`, "GraphEditing");
    return { status: "failed" };
  }
}
