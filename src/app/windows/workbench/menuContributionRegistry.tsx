import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import {
  useEditorHistoryAvailability,
  type WorkbenchCommandCapability,
} from "@/features/application/editor";
import { EDITOR_MUTATION_CAPABILITIES } from "@/features/application/editor/editorMutationAvailability";
import { useMenubar } from "@/features/application/menubar";
import { buildViewMenuItems } from "@/features/application/menubar/menubarViewItems";
import { useActiveProjectPath } from "@/features/application/project/projectSession";
import { useApplicationAppearance } from "@/features/application/settings/applicationSettings";
import { getRememberedColorTheme } from "@/features/application/settings/colorThemePresets";
import {
  openExternalUrlWithDialog,
  useCurrentWindowActions,
  useCustomTitleBar,
} from "@/features/application/window";
import {
  AboutModal,
  WorkbenchMenuBar,
  type WorkbenchMenuDefinition,
  type WorkbenchMenuItem,
} from "@/modules/workbench/public";
import { APP_LINKS } from "@/shared/config-default";

export type MenuItem = WorkbenchMenuItem;

export function buildEditMenuItems(
  translate: (key: string) => string,
  state: {
    activeResourceRef: string | null;
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
      onClick: authorized && EDITOR_MUTATION_CAPABILITIES.pasteNodes ? actions.paste : undefined,
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
    addEvent: () => void;
    addFunction: () => void;
    openProject: () => void;
    closeProject: () => void;
    saveGraph: () => void;
    saveGraphAs: () => void;
  },
): MenuItem[] {
  return [
    {
      label: translate("menubar.newEventGraph"),
      shortcut: "Ctrl+N",
      onClick: actions.addEvent,
    },
    { label: translate("menubar.newFunction"), onClick: actions.addFunction },
    { label: "-", type: "separator" },
    {
      label: translate("menubar.openProject"),
      shortcut: "Ctrl+O",
      onClick: actions.openProject,
    },
    { label: translate("menubar.closeProject"), onClick: actions.closeProject },
    { label: "-", type: "separator" },
    {
      label: translate("menubar.saveProject"),
      shortcut: "Ctrl+S",
      onClick:
        state.projectAvailable && state.editorCommandAuthorized ? actions.saveGraph : undefined,
    },
    {
      label: translate("menubar.saveProjectAs"),
      shortcut: "Ctrl+Shift+S",
      onClick: state.projectAvailable ? actions.saveGraphAs : undefined,
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
  const [aboutOpen, setAboutOpen] = useState(false);
  const {
    importGraph,
    saveGraph,
    saveGraphAs,
    undo,
    redo,
    copy,
    paste,
    cut,
    deleteSelected,
    addEvent,
    addFunction,
    addChart,
  } = commands;
  const { canUndo, canRedo, activeResourceRef } = useEditorHistoryAvailability();
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
  const { themeMode, appearance, updateAppearance } = useApplicationAppearance();
  const isLightTheme = themeMode === "light";
  const windowControls = useCurrentWindowActions();
  const customChrome = useCustomTitleBar();

  const toggleThemeMode = () => {
    const nextMode = isLightTheme ? "dark" : "light";
    updateAppearance({
      colorTheme: getRememberedColorTheme(
        nextMode,
        appearance.lastLightColorTheme,
        appearance.lastDarkColorTheme,
      ),
    });
  };

  const fileItems = buildFileMenuItems(
    t,
    { projectAvailable, editorCommandAuthorized },
    {
      addEvent: () => void addEvent(undefined, { openAfterCreate: true }),
      addFunction: () => void addFunction(undefined, { openAfterCreate: true }),
      openProject: () => void importGraph(),
      closeProject: () => navigate("/projects"),
      saveGraph: () => void saveGraph(),
      saveGraphAs: () => void saveGraphAs(),
    },
  );

  const editItems = buildEditMenuItems(
    t,
    { activeResourceRef, canUndo, canRedo, editorCommandAuthorized },
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
    { label: t("menubar.manageVariables") },
    { label: t("menubar.importData"), onClick: handleImportData },
    { label: t("menubar.databaseEditor"), onClick: handleDatabaseEditor },
    { label: t("menubar.newChart"), onClick: () => void addChart() },
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
          onToggle: toggleThemeMode,
        }}
        windowControls={windowControls}
      />
      <AboutModal
        open={aboutOpen}
        onOpenChange={setAboutOpen}
        onOpenRepository={() => void openExternalUrlWithDialog(APP_LINKS.repository, t)}
        onReportIssue={() => void openExternalUrlWithDialog(APP_LINKS.reportIssue, t)}
      />
    </>
  );
}
