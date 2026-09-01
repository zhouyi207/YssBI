import { markGraphTabDirty } from "@/features/core/layout/tabDirty";
import type { VariableScope } from "@/shared/types/domain/variable";

export function markVariableScopeDirty(scope: VariableScope): void {
  if (scope.type === "event") markGraphTabDirty(scope.eventPath);
  if (scope.type === "function") markGraphTabDirty(scope.functionPath);
}
