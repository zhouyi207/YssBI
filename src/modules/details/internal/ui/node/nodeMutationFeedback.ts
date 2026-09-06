import { graphDraftErrorMessageKey } from "@/features/application/graphDraft/graphDraftError";
import type { GraphEditOutcome } from "@/features/application/graphEditing/types";

export function graphDraftMutationSucceeded(result: GraphEditOutcome): boolean {
  return result.status === "applied" || result.status === "noop";
}

export function graphDraftMutationMessageKey(
  result: GraphEditOutcome,
  fallbackKey: string,
): string | null {
  if (graphDraftMutationSucceeded(result)) return null;
  const code = result.status === "rejected" ? result.code : null;
  return code ? (graphDraftErrorMessageKey(code) ?? fallbackKey) : fallbackKey;
}
