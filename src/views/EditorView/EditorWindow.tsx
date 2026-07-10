import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
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
  useEditorGroup,
  useEditorKeyboard,
} from "@/features/application/editor";
import { useMenubar } from "@/features/application/menubar";
import { toggleSidebarVisibility } from "@/features/core/layout/workbenchLayoutService";
import { toggleZenMode } from "@/features/core/layout/workbenchZenMode";
import { useWorkbenchLayout } from "@/features/application/layout/useWorkbenchLayout";
import { useAppearanceSettings } from "@/features/application/settings/useAppearanceSettings";
import { usePersistedWindow, usePersistedSecondaryWindow } from "@/features/application/window";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { SettingsView } from "./Layout/SettingsView";
import { ZenModeHintOverlay } from "./Layout/ZenModeHintOverlay";

function EditorWindowReady() {
  const rootId = useLayoutStore((s) => s.rootId);
  const isSettingsOpen = useLayoutStore((s) => s.isSettingsOpen);
  const zenMode = useLayoutStore((s) => s.zenMode);
  const setSettingsOpen = useLayoutStore((s) => s.setSettingsOpen);
  const activityBarPosition = useSettingsStore((s) => s.appearance.activityBarPosition);

  useWorkbenchLayout();
  useAppearanceSettings();
  useProjectSyncWithEditor();

  const editor = useEditorGroup();
  const { toggleLogPanel, toggleDetail } = useMenubar();
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
    toggleSidebar: toggleSidebarVisibility,
    toggleDetail,
    toggleZenMode,
  });

  const showActivityBar = !zenMode && activityBarPosition !== "Hidden";
  const activityBarOnRight = activityBarPosition === "Right";

  return (
    <div className="flex flex-col w-full h-screen">
      {!zenMode ? <Menubar /> : null}
      <div className="flex flex-1 overflow-hidden isolate">
        {showActivityBar && !activityBarOnRight ? <ActivityBar side="left" /> : null}
        <Workspace nodeId={rootId} />
        {showActivityBar && activityBarOnRight ? <ActivityBar side="right" /> : null}
      </div>
      {!zenMode ? <BottomBar /> : null}
      <ZenModeHintOverlay />
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
  usePersistedSecondaryWindow();

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
