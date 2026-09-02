import { executeCommandOutcome } from "@/features/core/history";
import type { GraphDraftCommandInvocation } from "@/features/core/history/commandExecutor";
import type { GraphDraftCommandResult } from "@/features/core/history/types";
import { logger } from "@/features/application/observability/appLogger";

export async function executeSafeGraphDraftEditOutcome(
  graphPath: string,
  operation: string,
  ...invocation: GraphDraftCommandInvocation
): Promise<GraphDraftCommandResult> {
  try {
    const outcome = await executeCommandOutcome(graphPath, ...invocation);
    if (outcome === false) return false;
    const code = outcome.status === "rejected" ? outcome.code : null;
    if (code) {
      logger.graph.warn(
        `Graph mutation outcome code=${code} graphPath=${graphPath} operation=${operation}`,
        "GraphDraft",
      );
    }
    return outcome;
  } catch {
    logger.graph.warn(
      `Graph mutation command failed graphPath=${graphPath} operation=${operation}`,
      "GraphDraft",
    );
    return false;
  }
}

export async function executeSafeGraphDraftEdit(
  graphPath: string,
  operation: string,
  ...invocation: GraphDraftCommandInvocation
): Promise<boolean> {
  const outcome = await executeSafeGraphDraftEditOutcome(graphPath, operation, ...invocation);
  return outcome !== false && outcome.status === "applied";
}
