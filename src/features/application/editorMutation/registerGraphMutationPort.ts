import { registerGraphMutationPort } from "@/features/core/history/graphMutationPort";
import { currentProjectionLocale } from "@/features/application/editorProjection/graphProjectionCoordinator";
import { executeEditorMutation } from "./editorMutationCoordinator";

let registered = false;

export function ensureGraphMutationPortRegistered(): void {
  if (registered) return;
  registerGraphMutationPort({
    execute: (graphPath, mutation) =>
      executeEditorMutation({
        graphPath,
        locale: currentProjectionLocale(),
        mutation,
      }),
  });
  registered = true;
}
