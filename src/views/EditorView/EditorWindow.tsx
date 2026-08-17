import { useState } from 'react';
import { useWorkbenchStore } from '@/features/core/workbench';
import { useTranslation } from "react-i18next";
import { ActivityBar } from "./Layout/ActivityBar";
import { BottomBar } from "./Layout/BottomBar";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { useProjectSync } from "@/features/application/initialization";
import {
  EditorSessionProvider,
  useEditorGroup,
  useEditorKeyboard,
} from "@/features/application/editor";
import { useMenubar } from "@/features/application/menubar";
import { toggleSidebarVisibility } from "@/features/core/layout/workbenchLayoutService";
import { toggleZenMode } from "@/features/core/layout/workbenchZenMode";
import { useWorkbenchLayout } from "@/features/application/layout/useWorkbenchLayout";
import { useEditorWorkbenchAppearance } from "@/features/application/settings/useEditorWorkbenchAppearance";
import { useActivityBarLayout } from "@/features/application/settings/useActivityBarLayout";
import { useEditorWindowGeometryPersistence } from "@/features/application/window";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { SettingsView } from "./Layout/SettingsView";
import { ZenModeHintOverlay } from "./Layout/ZenModeHintOverlay";
import { NodeDocumentationModal } from "./Layout/NodeDocumentationModal";
import { useProjectionLocaleSync } from "@/features/application/editor/useProjectionLocaleSync";

function EditorWindowReady() {
  const isSettingsOpen = useWorkbenchStore((state) => state.isSettingsOpen);
  const isNodeDocumentationOpen = useWorkbenchStore((state) => state.isNodeDocumentationOpen);
  const zenMode = useWorkbenchStore((state) => state.zenMode);
  const setSettingsOpen = useWorkbenchStore((state) => state.setSettingsOpen);
  const setNodeDocumentationOpen = useWorkbenchStore((state) => state.setNodeDocumentationOpen);
  const activityBar = useActivityBarLayout(zenMode);
  const [, setSettingsDirty] = useState(false);

  useWorkbenchLayout();
  useEditorWorkbenchAppearance();
  useProjectSync();
  useProjectionLocaleSync();

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
    selectAllNodes: editor.selectAllNodes,
    focusSelectedNodes: editor.focusSelectedNodes,
    fitCompleteGraph: editor.fitCompleteGraph,
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

  const showActivityBar = activityBar.visible;
  const activityBarOnRight = activityBar.side === "right";

  return (
    <div className="flex h-screen w-full flex-col bg-[var(--workbench-bg)] text-foreground" data-yssbi-workbench>
      {!zenMode ? <Menubar /> : null}
      <div className="isolate flex min-h-0 flex-1 overflow-hidden">
        {showActivityBar && !activityBarOnRight ? <ActivityBar side="left" /> : null}
        <Workspace />
        {showActivityBar && activityBarOnRight ? <ActivityBar side="right" /> : null}
      </div>
      {!zenMode ? <BottomBar /> : null}
      <ZenModeHintOverlay />
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
          className="h-[min(760px,86vh)] max-w-[min(1120px,92vw)] p-0"
        >
          <SettingsView
            onRequestClose={() => setSettingsOpen(false)}
            onDirtyChange={setSettingsDirty}
          />
        </DialogContent>
      </Dialog>
    </div>
  );
}

export const EditorWindow = () => {
  const { t } = useTranslation();
  const { status, error } = useAppInitialization();

  useEditorWindowGeometryPersistence();

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
