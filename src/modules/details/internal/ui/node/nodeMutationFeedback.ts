import { graphEditErrorMessageKey } from "@/features/application/graphEditing/graphEditError";
import type { GraphEditOutcome } from "@/features/application/graphEditing/types";

export function graphMutationSucceeded(result: GraphEditOutcome): boolean {
  return result.status === "applied" || result.status === "noop";
}

export function graphMutationMessageKey(
  result: GraphEditOutcome,
  fallbackKey: string,
): string | null {
  if (graphMutationSucceeded(result)) return null;
  const code = result.status === "rejected" ? result.code : null;
  return code ? (graphEditErrorMessageKey(code) ?? fallbackKey) : fallbackKey;
}
