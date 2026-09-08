import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { useTranslation } from "react-i18next";
import { useWorkbenchUi, workbenchUi } from "../../state/ui";

import type { WorkbenchOverlayRegistry } from "./overlayContribution";

export function WorkbenchOverlayHost({
  overlays,
}: {
  readonly overlays: WorkbenchOverlayRegistry;
}) {
  const { t } = useTranslation();
  const isSettingsOpen = useWorkbenchUi((state) => state.isSettingsOpen);
  const isNodeDocumentationOpen = useWorkbenchUi((state) => state.isNodeDocumentationOpen);
  const SettingsOverlay = overlays.settings;
  const NodeDocumentationOverlay = overlays.nodeDocumentation;

  return (
    <>
      <NodeDocumentationOverlay
        open={isNodeDocumentationOpen}
        onOpenChange={workbenchUi.setNodeDocumentationOpen}
      />
      <Dialog open={isSettingsOpen} onOpenChange={workbenchUi.setSettingsOpen}>
        <DialogContent
          aria-describedby={undefined}
          className="h-[min(720px,88dvh)] max-w-[min(1000px,92vw)] grid-rows-[minmax(0,1fr)] rounded-md p-0 motion-reduce:animate-none max-[720px]:h-[92dvh] max-[720px]:max-w-[96vw]"
        >
          <DialogTitle className="sr-only">{t("settings.title")}</DialogTitle>
          <SettingsOverlay onRequestClose={() => workbenchUi.setSettingsOpen(false)} />
        </DialogContent>
      </Dialog>
    </>
  );
}
