import { useWorkbenchUi, workbenchUi } from '@/features/core/workbench/ui';
import { useTranslation } from "react-i18next";

import { BottomBar } from "./Layout/BottomBar";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { WorkbenchActivityPanelsProvider } from './Layout/WorkbenchActivityPanels';
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { useProjectSync } from "@/features/application/initialization";
import {
  EditorSessionProvider,
  useEditorKeyboard,
  useEditorWindowCloseGuard,
} from "@/features/application/editor";
import { useWorkbenchLayout } from "@/features/application/layout/useWorkbenchLayout";

import { useEditorWindowGeometryPersistence } from "@/features/application/window";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { SettingsView } from "./Layout/SettingsView";
import { NodeDocumentationModal } from "./Layout/NodeDocumentationModal";
import { useProjectionLocaleSync } from "@/features/application/editor/useProjectionLocaleSync";

function EditorWindowReady() {
  const isSettingsOpen = useWorkbenchUi((state) => state.isSettingsOpen);
  const isNodeDocumentationOpen = useWorkbenchUi((state) => state.isNodeDocumentationOpen);
  const setSettingsOpen = workbenchUi.setSettingsOpen;
  const setNodeDocumentationOpen = workbenchUi.setNodeDocumentationOpen;
  useWorkbenchLayout();
  useProjectSync();
  useProjectionLocaleSync();

  useEditorKeyboard();


  return (
    <div className="flex h-screen w-full flex-col bg-[var(--workbench-bg)] text-foreground" data-yssbi-workbench>
      <Menubar />
      <div className="isolate flex min-h-0 flex-1 overflow-hidden">
        <WorkbenchActivityPanelsProvider>
          <Workspace />
        </WorkbenchActivityPanelsProvider>
      </div>
      <BottomBar />
      <NodeDocumentationModal open={isNodeDocumentationOpen} onOpenChange={setNodeDocumentationOpen} />
      <Dialog
        open={isSettingsOpen}
        onOpenChange={(open) => {
          if (open) setSettingsOpen(true);
        }}
      >
        <DialogContent
          explicitClose
          onEscapeKeyDown={(event) => event.preventDefault()}
          className="h-[min(760px,86vh)] max-w-[min(1120px,92vw)] p-0 max-[720px]:h-[92vh] max-[720px]:max-w-[96vw]"
        >
          <SettingsView onRequestClose={() => setSettingsOpen(false)} />
        </DialogContent>
      </Dialog>
    </div>
  );
}

export const EditorWindow = () => {
  const { t } = useTranslation();
  const { status, error } = useAppInitialization();

  useEditorWindowGeometryPersistence();
  useEditorWindowCloseGuard();

  if (status !== LoadStatus.Ready) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-[var(--workbench-bg)] text-sm text-muted-foreground">
        <div className="flex items-center gap-3 rounded-lg border border-[var(--strong-border)] bg-[var(--surface-raised)] px-4 py-3 shadow-lg">
          {!error ? <span className="size-2 animate-pulse rounded-full bg-[var(--accent-color)]" /> : null}
          {error ? t("editor.initializationFailed", { error }) : t("common.loading")}
        </div>
      </div>
    );
  }

  return (
    <EditorSessionProvider>
      <EditorWindowReady />
    </EditorSessionProvider>
  );
};
