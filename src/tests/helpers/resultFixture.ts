import type { ResultReference } from "@/shared/types/domain/result";

export const resultSessionFixture = "00000000-0000-0000-0000-000000000001";

export function resultReferenceFixture(
  resultId: string,
  executionSessionId = resultSessionFixture,
): ResultReference {
  return { executionSessionId, resultId };
}

export function resultLeaseIdFixture(index: number): string {
  return `00000000-0000-0000-0000-${String(index).padStart(12, "0")}`;
}
