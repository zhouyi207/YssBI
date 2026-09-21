import { useWorkbenchUi, workbenchUi } from "../../state/ui";

import type { WorkbenchOverlayRegistry } from "./overlayContribution";

export function WorkbenchOverlayHost({
  overlays,
}: {
  readonly overlays: WorkbenchOverlayRegistry;
}) {
  const isNodeDocumentationOpen = useWorkbenchUi((state) => state.isNodeDocumentationOpen);
  const NodeDocumentationOverlay = overlays.nodeDocumentation;

  return (
    <NodeDocumentationOverlay
      open={isNodeDocumentationOpen}
      onOpenChange={workbenchUi.setNodeDocumentationOpen}
    />
  );
}
