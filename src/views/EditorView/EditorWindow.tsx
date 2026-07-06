import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useTranslation } from "react-i18next";
import { ActivityBar } from "./Layout/ActivityBar";
import { BottomBar } from "./Layout/BottomBar";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { useProjectSyncWithEditor } from "@/features/application/initialization";
import {
  EditorSessionProvider,
  useAutoOpenFirstGraph,
  useEditorGroup,
  useEditorKeyboard,
} from "@/features/application/editor";
import { useMenubar } from "@/features/application/menubar";
import { usePersistedWindow } from "@/features/application/window";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { SettingsView } from "./Layout/SettingsView";

function EditorWindowReady() {
  const { t } = useTranslation();
  const rootId = useLayoutStore((s) => s.rootId);
  const isSettingsOpen = useLayoutStore((s) => s.isSettingsOpen);
  const setSettingsOpen = useLayoutStore((s) => s.setSettingsOpen);

  useProjectSyncWithEditor();
  useAutoOpenFirstGraph();

  const editor = useEditorGroup();
  const { toggleLogPanel } = useMenubar();
  useEditorKeyboard({
    deleteSelected: editor.deleteSelected,
    undo: editor.undo,
    redo: editor.redo,
    copy: editor.copy,
    cut: editor.cut,
    paste: editor.paste,
    duplicateSelected: editor.duplicateSelected,
    saveGraph: editor.saveGraph,
    saveGraphAs: editor.saveGraphAs,
    importGraph: editor.importGraph,
    addEvent: editor.addEvent,
    closeTab: editor.closeTab,
    setActiveTabId: editor.setActiveTabId,
    splitEditorRight: editor.splitEditorRight,
    toggleLogPanel,
  });

  return (
    <div className="flex flex-col w-full h-screen">
      <Menubar />
      <div className="flex flex-1 overflow-hidden isolate">
        <ActivityBar />
        <Workspace nodeId={rootId} />
      </div>
      <BottomBar />
      <Dialog open={isSettingsOpen} onOpenChange={setSettingsOpen}>
        <DialogContent className="h-[min(760px,86vh)] max-w-[min(1120px,92vw)] p-0">
          <SettingsView />
        </DialogContent>
      </Dialog>
    </div>
  );
}

export const EditorWindow = () => {
  const { t } = useTranslation();
  const { status, error } = useAppInitialization();

  usePersistedWindow("main");

  if (status !== LoadStatus.Ready) {
    return (
      <div className="flex items-center justify-center w-full h-screen">
        {error ? t("editor.initializationFailed", { error }) : t("common.loading")}
      </div>
    );
  }

  return (
    <EditorSessionProvider>
      <EditorWindowReady />
    </EditorSessionProvider>
  );
};
