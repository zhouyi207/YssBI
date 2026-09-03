import { graphDraftErrorMessageKey } from "@/features/application/graphDraft/graphDraftError";
import type { GraphDraftCommandResult } from "@/features/core/history/types";

export function graphDraftMutationSucceeded(result: GraphDraftCommandResult): boolean {
  return result !== false && (result.status === "applied" || result.status === "noop");
}

export function graphDraftMutationMessageKey(
  result: GraphDraftCommandResult,
  fallbackKey: string,
): string | null {
  if (graphDraftMutationSucceeded(result)) return null;
  const code = result !== false && result.status === "rejected" ? result.code : null;
  return code ? (graphDraftErrorMessageKey(code) ?? fallbackKey) : fallbackKey;
}
