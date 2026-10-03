import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router";

import {
  useEditorHistoryAvailability,
  type WorkbenchCommandCapability,
} from "@/features/application/editor";
import { useMenubar } from "@/features/application/menubar";
import { buildViewMenuItems } from "@/features/application/menubar/menubarViewItems";
import { useActiveProjectPath } from "@/features/application/project/projectSession";
import { requestCloseProject } from "@/features/application/project/closeProject";
import {
  toggleApplicationThemeMode,
  useApplicationThemeMode,
} from "@/features/application/settings/applicationSettings";
import {
  openExternalUrlWithDialog,
  useCurrentWindowActions,
  useCustomTitleBar,
} from "@/features/application/window";
import {
  AboutModal,
  ArchitectureModal,
  architectureSearch,
  WorkbenchMenuBar,
  type WorkbenchMenuDefinition,
  type WorkbenchMenuItem,
} from "@/modules/workbench/public";
import { APP_LINKS } from "@/shared/config-default";

export type MenuItem = WorkbenchMenuItem;

export function buildEditMenuItems(
  translate: (key: string) => string,
  state: {
    canUndo: boolean;
    canRedo: boolean;
    editorCommandAuthorized: boolean;
  },
  actions: {
    undo: () => void;
    redo: () => void;
    cut: () => void;
    copy: () => void;
    paste: () => void;
    deleteSelected: () => void;
  },
): MenuItem[] {
  const authorized = state.editorCommandAuthorized;
  return [
    {
      label: translate("common.undo"),
      shortcut: "Ctrl+Z",
      onClick: authorized && state.canUndo ? actions.undo : undefined,
    },
    {
      label: translate("common.redo"),
      shortcut: "Ctrl+Y",
      onClick: authorized && state.canRedo ? actions.redo : undefined,
    },
    { label: "-", type: "separator" },
    {
      label: translate("menubar.cut"),
      shortcut: "Ctrl+X",
      onClick: authorized ? actions.cut : undefined,
    },
    {
      label: translate("menubar.copy"),
      shortcut: "Ctrl+C",
      onClick: authorized ? actions.copy : undefined,
    },
    {
      label: translate("menubar.paste"),
      shortcut: "Ctrl+V",
      onClick: authorized ? actions.paste : undefined,
    },
    { label: "-", type: "separator" },
    {
      label: translate("common.delete"),
      shortcut: "Del",
      onClick: authorized ? actions.deleteSelected : undefined,
    },
  ];
}

export function buildFileMenuItems(
  translate: (key: string) => string,
  state: {
    projectAvailable: boolean;
    editorCommandAuthorized: boolean;
  },
  actions: {
    createFile: (kind: import("@/shared/types/domain/resource").FileResourceKind) => void;
    openProject: () => void;
    closeProject: () => void;
    saveActiveFile: () => void;
    saveProjectAs: () => void;
  },
): MenuItem[] {
  return [
    {
      label: translate("menubar.newEventGraph"),
      shortcut: "Ctrl+N",
      onClick: () => actions.createFile("event_graph"),
    },
    {
      label: translate("menubar.newFunctionGraph"),
      onClick: () => actions.createFile("function_graph"),
    },
    { label: translate("menubar.newChart"), onClick: () => actions.createFile("chart") },
    { label: translate("documents.newMind"), onClick: () => actions.createFile("mind") },
    { label: translate("documents.newDoc"), onClick: () => actions.createFile("doc") },
    { label: "-", type: "separator" },
    {
      label: translate("menubar.openProject"),
      shortcut: "Ctrl+O",
      onClick: actions.openProject,
    },
    { label: translate("menubar.closeProject"), onClick: actions.closeProject },
    { label: "-", type: "separator" },
    {
      label: translate("common.save"),
      shortcut: "Ctrl+S",
      onClick:
        state.projectAvailable && state.editorCommandAuthorized
          ? actions.saveActiveFile
          : undefined,
    },
    {
      label: translate("menubar.saveProjectAs"),
      shortcut: "Ctrl+Shift+S",
      onClick: state.projectAvailable ? actions.saveProjectAs : undefined,
    },
  ];
}

export function buildWindowMenuItems(
  translate: (key: string) => string,
  editorCommandAuthorized: boolean,
  actions: {
    splitRight: () => void;
    splitDown: () => void;
    openLogsWindow: () => void;
  },
): MenuItem[] {
  return [
    {
      label: translate("menubar.splitEditorRight"),
      onClick: editorCommandAuthorized ? actions.splitRight : undefined,
    },
    {
      label: translate("menubar.splitEditorDown"),
      onClick: editorCommandAuthorized ? actions.splitDown : undefined,
    },
    { label: "-", type: "separator" },
    {
      label: translate("menubar.openLogsInNewWindow"),
      onClick: actions.openLogsWindow,
    },
  ];
}

export function WorkbenchMenuContribution({
  commands,
}: {
  readonly commands: WorkbenchCommandCapability;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const [aboutOpen, setAboutOpen] = useState(false);
  const {
    openProject,
    saveActiveFile,
    saveProjectAs,
    undo,
    redo,
    copy,
    paste,
    cut,
    deleteSelected,
    createFile,
  } = commands;
  const { canUndo, canRedo } = useEditorHistoryAvailability();
  const {
    openSettings,
    editorCommandAuthorized,
    viewState,
    viewActions,
    handleImportData,
    handleSplitRight,
    handleSplitDown,
    handleDatabaseEditor,
    handleOpenLogs,
  } = useMenubar();
  const currentPath = useActiveProjectPath();
  const projectAvailable = Boolean(currentPath);
  const themeMode = useApplicationThemeMode();
  const isLightTheme = themeMode === "light";
  const windowControls = useCurrentWindowActions();
  const customChrome = useCustomTitleBar();

  const fileItems = buildFileMenuItems(
    t,
    { projectAvailable, editorCommandAuthorized },
    {
      createFile: (kind) => void createFile(kind),
      openProject: () => void openProject(),
      closeProject: () => {
        void requestCloseProject().then((closed) => {
          if (closed) navigate("/projects", { replace: true });
        });
      },
      saveActiveFile: () => void saveActiveFile(),
      saveProjectAs: () => void saveProjectAs(),
    },
  );

  const editItems = buildEditMenuItems(
    t,
    { canUndo, canRedo, editorCommandAuthorized },
    {
      undo: () => void undo(),
      redo: () => void redo(),
      cut: () => void cut(),
      copy: () => void copy(),
      paste: () => void paste(),
      deleteSelected: () => void deleteSelected(),
    },
  );

  const dataItems: MenuItem[] = [
    { label: t("menubar.importData"), onClick: handleImportData },
    { label: t("menubar.databaseEditor"), onClick: handleDatabaseEditor },
    { label: "-", type: "separator" },
    { label: t("menubar.schemaViewer") },
  ];

  const windowItems = buildWindowMenuItems(t, editorCommandAuthorized, {
    splitRight: handleSplitRight,
    splitDown: handleSplitDown,
    openLogsWindow: handleOpenLogs,
  });
  const toolItems: MenuItem[] = [
    { label: t("menubar.debugger") },
    { label: t("menubar.profiler") },
    { label: "-", type: "separator" },
    { label: t("menubar.settings"), shortcut: "Ctrl+,", onClick: openSettings },
  ];
  const helpItems: MenuItem[] = [
    {
      label: t("menubar.architecture"),
      onClick: () =>
        navigate({ ...location, search: architectureSearch(location.search, "overview") }),
    },
    {
      label: t("menubar.documentation"),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.documentation, t),
    },
    { label: "-", type: "separator" },
    {
      label: t("menubar.releaseNotes"),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.releaseNotes, t),
    },
    {
      label: t("menubar.githubRepository"),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.repository, t),
    },
    {
      label: t("menubar.reportIssue"),
      onClick: () => void openExternalUrlWithDialog(APP_LINKS.reportIssue, t),
    },
    { label: "-", type: "separator" },
    { label: t("menubar.about"), onClick: () => setAboutOpen(true) },
  ];
  const menus: WorkbenchMenuDefinition[] = [
    { id: "file", label: t("menubar.file"), items: fileItems },
    { id: "edit", label: t("menubar.edit"), items: editItems },
    { id: "data", label: t("menubar.data"), items: dataItems },
    { id: "view", label: t("menubar.view"), items: buildViewMenuItems(t, viewState, viewActions) },
    { id: "window", label: t("menubar.window"), items: windowItems },
    { id: "tools", label: t("menubar.tools"), items: toolItems },
    { id: "help", label: t("menubar.help"), items: helpItems },
  ];

  return (
    <>
      <WorkbenchMenuBar
        menus={menus}
        customChrome={customChrome}
        themeToggle={{
          isLightTheme,
          label: isLightTheme ? t("menubar.switchToDark") : t("menubar.switchToLight"),
          onToggle: toggleApplicationThemeMode,
        }}
        windowControls={windowControls}
      />
      <AboutModal
        open={aboutOpen}
        onOpenChange={setAboutOpen}
        onOpenRepository={() => void openExternalUrlWithDialog(APP_LINKS.repository, t)}
        onReportIssue={() => void openExternalUrlWithDialog(APP_LINKS.reportIssue, t)}
      />
      <ArchitectureModal />
    </>
  );
}
