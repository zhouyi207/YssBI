import { useTranslation } from "react-i18next";

import { BottomBar } from "./Layout/BottomBar";
import { Menubar } from "./Layout/Menubar";
import { RootDockviewHost, type RootPanelRegistry } from "./Layout/RootDockviewHost";
import { WorkbenchActivityPanelsProvider } from "./Layout/WorkbenchActivityPanels";
import { WorkbenchOverlayHost } from "./Layout/WorkbenchOverlayHost";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { useProjectSync } from "@/features/application/initialization";
import {
  EditorSessionProvider,
  useEditorKeyboard,
  useWorkbenchWindowCloseGuard,
} from "@/features/application/editor";

import { useWorkbenchWindowGeometryPersistence } from "@/features/application/window";
import { useProjectionLocaleSync } from "@/features/application/editor/useProjectionLocaleSync";

function WorkbenchWindowReady({ panelRegistry }: { readonly panelRegistry: RootPanelRegistry }) {
  useProjectSync();
  useProjectionLocaleSync();

  useEditorKeyboard();

  return (
    <div
      className="flex h-screen w-full flex-col bg-[var(--workbench-bg)] text-foreground"
      data-yssbi-workbench
    >
      <Menubar />
      <div className="isolate flex min-h-0 flex-1 overflow-hidden">
        <WorkbenchActivityPanelsProvider>
          <RootDockviewHost panelRegistry={panelRegistry} />
        </WorkbenchActivityPanelsProvider>
      </div>
      <BottomBar />
      <WorkbenchOverlayHost />
    </div>
  );
}

export function WorkbenchWindow({ panelRegistry }: { readonly panelRegistry: RootPanelRegistry }) {
  const { t } = useTranslation();
  const { status, error } = useAppInitialization();

  useWorkbenchWindowGeometryPersistence();
  useWorkbenchWindowCloseGuard();

  if (status !== LoadStatus.Ready) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-[var(--workbench-bg)] text-sm text-muted-foreground">
        <div className="flex items-center gap-3 rounded-lg border border-[var(--strong-border)] bg-[var(--surface-raised)] px-4 py-3 shadow-lg">
          {!error ? (
            <span className="size-2 animate-pulse rounded-full bg-[var(--accent-color)]" />
          ) : null}
          {error ? t("editor.initializationFailed", { error }) : t("common.loading")}
        </div>
      </div>
    );
  }

  return (
    <EditorSessionProvider>
      <WorkbenchWindowReady panelRegistry={panelRegistry} />
    </EditorSessionProvider>
  );
}
