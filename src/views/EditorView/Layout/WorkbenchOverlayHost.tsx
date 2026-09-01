import { Dialog, DialogContent } from "@/components/ui/dialog";
import { useWorkbenchUi, workbenchUi } from "@/features/core/workbench/ui";

import { NodeDocumentationModal } from "./NodeDocumentationModal";
import { SettingsView } from "./SettingsView";

export function WorkbenchOverlayHost() {
  const isSettingsOpen = useWorkbenchUi((state) => state.isSettingsOpen);
  const isNodeDocumentationOpen = useWorkbenchUi((state) => state.isNodeDocumentationOpen);

  return (
    <>
      <NodeDocumentationModal
        open={isNodeDocumentationOpen}
        onOpenChange={workbenchUi.setNodeDocumentationOpen}
      />
      <Dialog
        open={isSettingsOpen}
        onOpenChange={(open) => {
          if (open) workbenchUi.setSettingsOpen(true);
        }}
      >
        <DialogContent
          explicitClose
          onEscapeKeyDown={(event) => event.preventDefault()}
          className="h-[min(760px,86vh)] max-w-[min(1120px,92vw)] p-0 max-[720px]:h-[92vh] max-[720px]:max-w-[96vw]"
        >
          <SettingsView onRequestClose={() => workbenchUi.setSettingsOpen(false)} />
        </DialogContent>
      </Dialog>
    </>
  );
}
