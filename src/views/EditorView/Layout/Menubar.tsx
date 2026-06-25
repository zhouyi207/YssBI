import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { useEditorGroup } from "@/features/application/editor";
import { VscLayoutSidebarRight, VscLayoutSidebarRightOff, VscSettingsGear } from "react-icons/vsc";
import { useMenubar } from "@/features/application/menubar";
import { useProjectIOStore } from "@/features/core/dataStore/projectIOStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME } from "@/app/appConfig/default";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import type { ThemeSettings } from "@/shared/types/settings";
import { cn } from "@/lib/utils";
import { useWindowMaximized } from "@/features/application/window";
import { WindowChromeControls } from "@/shared/ui/WindowChromeControls";
import { WindowTitleBar, WindowTitleBarActions } from "@/shared/ui/WindowTitleBar";

interface MenuItem {
  label: string;
  shortcut?: string;
  onClick?: () => void;
  type?: 'item' | 'separator';
}

interface MenuButtonProps {
  id: string;
  label: string;
  items: MenuItem[];
}

const MenuButton = ({ label, items }: MenuButtonProps) => {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="h-7 px-3 text-sm text-muted-foreground hover:text-foreground">
          {label}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-[190px]">
        {items.map((item, i) => {
          if (item.type === 'separator' || item.label === '-') {
            return <DropdownMenuSeparator key={`${label}-${i}`} />;
          }

          return (
            <DropdownMenuItem
              key={`${label}-${i}`}
              disabled={!item.onClick}
              onSelect={(event) => {
                if (!item.onClick) {
                  event.preventDefault();
                  return;
                }
                item.onClick();
              }}
              className="gap-8"
            >
              <span className="flex-1">{item.label}</span>
              {item.shortcut && <DropdownMenuShortcut>{item.shortcut}</DropdownMenuShortcut>}
            </DropdownMenuItem>
          );
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  );
};

const THEME_BASE_KEYS = [
  "mode",
  "workbenchBackground",
  "sidebarBackground",
  "gridLines",
  "nodeBase",
  "connectionLines",
  "selectionRegion",
  "execColor",
  "objectColor",
  "anyColor",
] satisfies Array<keyof ThemeSettings>;

function pickThemeBase(theme: ThemeSettings): Partial<ThemeSettings> {
  return Object.fromEntries(THEME_BASE_KEYS.map((key) => [key, theme[key]])) as Partial<ThemeSettings>;
}

export function Menubar() {
  const { t } = useTranslation();
  const navigate = useNavigate();
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
    canUndo,
    canRedo,
    activeTabId,
    addEvent,
    addFunction,
    addWorksheet,
  } = useEditorGroup({ withCanvasInteraction: false });

  const {
    openSettings,
    isDetailVisible,
    isLogPanelVisible,
    handleImportData,
    handleSplitRight,
    handleSplitDown,
    handleDataView,
    handleOpenLogs,
    toggleDetail,
    toggleLogPanel,
    openNewWindow,
  } = useMenubar();

  const currentPath = useProjectIOStore((s) => s.currentPath);
  const saveableEditorTabId = useLayoutStore((s) => {
    const editorGroupId = s.activeEditorGroupId || "default_editor";
    const tabId = s.nodes[editorGroupId]?.data?.activeTabId;
    if (!tabId || tabId === "settings") return null;
    const tab = s.nodes[editorGroupId]?.data?.tabs?.find((t) => t.id === tabId);
    if (
      tab &&
      tab.type !== "event" &&
      tab.type !== "function" &&
      tab.type !== "worksheet"
    ) {
      return null;
    }
    return tabId;
  });
  const canSaveProject = Boolean(currentPath && saveableEditorTabId);
  const canSaveProjectAs = Boolean(currentPath);

  const themeMode = useSettingsStore((s) => s.theme.mode ?? "dark");
  const updateTheme = useSettingsStore((s) => s.updateTheme);
  const saveDebounced = useSettingsStore((s) => s.saveDebounced);
  const isLightTheme = themeMode === "light";
  const isMaximized = useWindowMaximized("Menubar");
  const toggleThemeMode = () => {
    updateTheme(pickThemeBase(isLightTheme ? DEFAULT_DARK_THEME : DEFAULT_LIGHT_THEME));
    saveDebounced();
  };

  const fileItems: MenuItem[] = [
    { label: t("menubar.newEventGraph"), shortcut: "Ctrl+N", onClick: () => addEvent(undefined, { openAfterCreate: true }) },
    { label: t("menubar.newFunction"), onClick: () => addFunction(undefined, { openAfterCreate: true }) },
    { label: "-" },
    { label: t("menubar.openProject"), shortcut: "Ctrl+O", onClick: () => importGraph() },
    { label: t("menubar.closeProject"), onClick: () => navigate("/projects") },
    { label: "-" },
    { label: t("menubar.saveProject"), shortcut: "Ctrl+S", onClick: canSaveProject ? () => saveGraph() : undefined },
    { label: t("menubar.saveProjectAs"), shortcut: "Ctrl+Shift+S", onClick: canSaveProjectAs ? () => saveGraphAs() : undefined },
  ];

  const editItems: MenuItem[] = [
    { label: t("common.undo"), shortcut: "Ctrl+Z", onClick: (activeTabId && canUndo) ? undo : undefined },
    { label: t("common.redo"), shortcut: "Ctrl+Y", onClick: (activeTabId && canRedo) ? redo : undefined },
    { label: "-" },
    { label: t("menubar.cut"), shortcut: "Ctrl+X", onClick: activeTabId ? cut : undefined },
    { label: t("menubar.copy"), shortcut: "Ctrl+C", onClick: activeTabId ? copy : undefined },
    { label: t("menubar.paste"), shortcut: "Ctrl+V", onClick: activeTabId ? () => paste() : undefined },
    { label: "-" },
    { label: t("common.delete"), shortcut: "Del", onClick: activeTabId ? deleteSelected : undefined },
  ];

  const dataItems: MenuItem[] = [
    { label: t("menubar.manageVariables") },
    { label: t("menubar.importData"), onClick: handleImportData },
    { label: t("menubar.dataViewer"), onClick: handleDataView },
    { label: t("menubar.newWorksheet"), onClick: () => void addWorksheet() },
    { label: "-" },
    { label: t("menubar.schemaViewer") },
  ];

  const windowItems: MenuItem[] = [
    { label: t("menubar.newWindow"), onClick: openNewWindow },
    { label: "-" },
    { label: t("menubar.splitEditorRight"), onClick: handleSplitRight },
    { label: t("menubar.splitEditorDown"), onClick: handleSplitDown },
    { label: "-" },
    { label: isLogPanelVisible ? t("menubar.hideLogs") : t("menubar.showLogs"), shortcut: "Ctrl+`", onClick: toggleLogPanel },
    { label: t("menubar.openLogsInNewWindow"), onClick: handleOpenLogs },
    { label: "-" },
    { label: t("menubar.resetLayout") },
    { label: t("menubar.zoomIn"), shortcut: "Ctrl++" },
    { label: t("menubar.zoomOut"), shortcut: "Ctrl+-" },
  ];

  const toolItems: MenuItem[] = [
    { label: t("menubar.debugger") },
    { label: t("menubar.profiler") },
    { label: "-" },
    { label: t("menubar.settings"), shortcut: "Ctrl+,", onClick: openSettings },
  ];

  const helpItems: MenuItem[] = [
    { label: t("menubar.documentation") },
    { label: t("menubar.releaseNotes") },
    { label: t("menubar.about") },
  ];

  return (
    <WindowTitleBar
      elevated
      className="menubar-container"
      onWheel={(e) => e.stopPropagation()}
    >
      {/* Left: Icon & Brand */}
      <div className="flex items-center gap-2 px-4 pointer-events-none self-center">
        <div className="w-5 h-5 bg-[var(--accent-color)] rounded flex items-center justify-center">
          <span className="text-white font-black text-xs">Y</span>
        </div>
        <div className="text-foreground font-bold text-sm tracking-tight">
          Yss<span className="text-[var(--accent-color)]">BI</span>
        </div>
      </div>

      {/* Center Left: Menus */}
      <div className="flex items-center gap-1 self-center">
        <MenuButton id="file" label={t("menubar.file")} items={fileItems} />
        <MenuButton id="edit" label={t("menubar.edit")} items={editItems} />
        <MenuButton id="data" label={t("menubar.data")} items={dataItems} />
        <MenuButton id="window" label={t("menubar.window")} items={windowItems} />
        <MenuButton id="tools" label={t("menubar.tools")} items={toolItems} />
        <MenuButton id="help" label={t("menubar.help")} items={helpItems} />
      </div>

      <div className="flex-1 min-w-[20px]" data-tauri-drag-region />

      {/* Right side: Window Buttons */}
      <WindowTitleBarActions>
        {/* Theme Toggle Button */}
        <Button
          variant="ghost"
          size="icon-lg"
          onClick={toggleThemeMode}
          className="self-center text-muted-foreground"
          title={isLightTheme ? t("menubar.switchToDark") : t("menubar.switchToLight")}
          aria-label={isLightTheme ? t("menubar.switchToDark") : t("menubar.switchToLight")}
        >
          {isLightTheme ? (
            <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12.8A8.5 8.5 0 1111.2 3a7 7 0 009.8 9.8z" />
            </svg>
          ) : (
            <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 3v2m0 14v2m9-9h-2M5 12H3m15.36-6.36-1.42 1.42M7.06 16.94l-1.42 1.42m12.72 0-1.42-1.42M7.06 7.06 5.64 5.64" />
              <circle cx="12" cy="12" r="4" strokeWidth={2} />
            </svg>
          )}
        </Button>
        {/* Toggle Detail Button */}
        <Button
          variant="ghost"
          size="icon-lg"
          onClick={toggleDetail}
          className={cn(
            'self-center',
            isDetailVisible ? 'text-foreground' : 'text-muted-foreground',
          )}
          title={isDetailVisible ? t("menubar.hideDetail") : t("menubar.showDetail")}
        >
          {isDetailVisible ? <VscLayoutSidebarRight size={14} /> : <VscLayoutSidebarRightOff size={14} />}
        </Button>

        {/* Settings Button */}
        <Button
          variant="ghost"
          size="icon-lg"
          onClick={() => openSettings()}
          className="self-center text-muted-foreground"
          title={t("menubar.settings")}
        >
          <VscSettingsGear size={14} />
        </Button>
        <WindowChromeControls isMaximized={isMaximized} />
      </WindowTitleBarActions>
    </WindowTitleBar>
  );
}
