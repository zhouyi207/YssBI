import { registerGraphDraftPort } from "@/features/core/history/graphDraftPort";
import { currentProjectionLocale } from "@/features/application/editorProjection/graphProjectionCoordinator";
import { applyGraphDraftMutation } from "./graphDraftCoordinator";

let registered = false;

export function ensureGraphDraftPortRegistered(): void {
  if (registered) return;
  registerGraphDraftPort({
    execute: (graphPath, mutation) =>
      applyGraphDraftMutation({
        graphPath,
        locale: currentProjectionLocale(),
        mutation,
      }),
  });
  registered = true;
}
