import { markGraphEditorPanelDirty } from "@/features/core/layout/editorPanelDirty";
import type { VariableScope } from "@/shared/types/domain/variable";

export function markVariableScopeDirty(scope: VariableScope): void {
  if (scope.type === "event") markGraphEditorPanelDirty(scope.eventPath);
  if (scope.type === "function") markGraphEditorPanelDirty(scope.functionPath);
}
