import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import {
  VscClose,
  VscClearAll,
  VscDebugStart,
  VscFolder,
  VscFolderOpened,
  VscGithub,
  VscNewFile,
  VscProject,
  VscRefresh,
  VscSearch,
  VscSettingsGear,
  VscStarEmpty,
  VscStarFull,
  VscTrash,
  VscWarning,
} from "react-icons/vsc";
import { i18n, type AppLanguage } from "@/app/i18n";
import { APP_LINKS } from "@/app/appConfig/default";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { useProjectPicker, type ManagedProject } from "@/features/application/project";
import { useProjectIOStore } from "@/features/core/dataStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { usePersistedWindow, useWindowMaximized } from "@/features/application/window";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { ContextMenu, usePositionedContextMenu } from "@/shared/ui/contextMenu";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { ProjectService } from "@/services/project/projectService";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { WindowChromeControls } from "@/shared/ui/WindowChromeControls";
import { WindowMenuBar } from "@/shared/ui/WindowChrome";
import { openExternalUrl } from "@/shared/utils/openExternalUrl";
import { getRememberedColorTheme } from "@/features/application/settings/colorThemePresets";
import { NewProjectModal } from "./NewProjectModal";
import { DeleteProjectConfirmDialog } from "./DeleteProjectConfirmDialog";
import { buildProjectPickerContextMenuSections } from "./projectPickerContextMenu";
import type { ProjectPickerContextMenuTarget } from "./projectPickerContextMenu";

type SortMode = "lastOpened" | "name";

const APP_CARD_OUTER_CLASS = "border-border bg-card ring-1 ring-border";
function sortAndFilter(items: ManagedProject[], query: string, mode: SortMode): ManagedProject[] {
  const q = query.trim().toLowerCase();
  const list = q
    ? items.filter((project) =>
      project.name.toLowerCase().includes(q) ||
      project.path.toLowerCase().includes(q)
    )
    : items;

  return [...list].sort((a, b) => {
    const fa = a.isFavorite ? 1 : 0;
    const fb = b.isFavorite ? 1 : 0;
    if (fa !== fb) return fb - fa;
    if (mode === "name") return a.name.localeCompare(b.name, "zh");
    return b.lastOpenedAt.localeCompare(a.lastOpenedAt);
  });
}

function formatStamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function SidebarBtn({
  children,
  disabled,
  primary,
  danger,
  onClick,
}: {
  children: ReactNode;
  disabled?: boolean;
  primary?: boolean;
  danger?: boolean;
  onClick?: () => void;
}) {
  return (
    <Button
      type="button"
      disabled={disabled}
      onClick={onClick}
      variant={danger ? "destructive" : primary ? "default" : "outline"}
      className={cn(
        "h-auto w-full min-w-0 justify-start rounded px-3 py-2 text-[12px] font-medium",
        !primary && !danger && "border-border bg-muted/70 text-foreground hover:bg-muted",
      )}
    >
      {children}
    </Button>
  );
}

function TitleBar({
  filterQuery,
  sortMode,
  onGoEditor,
  onOpenSettings,
  onSetFilterQuery,
  onSetSortMode,
}: {
  filterQuery: string;
  sortMode: SortMode;
  onGoEditor: () => void;
  onOpenSettings: () => void;
  onSetFilterQuery: (value: string) => void;
  onSetSortMode: (value: SortMode) => void;
}) {
  const { t } = useTranslation();
  const currentPath = useProjectIOStore((state) => state.currentPath);
  const themeMode = useSettingsStore((state) => state.theme.mode ?? "dark");
  const appearance = useSettingsStore((state) => state.appearance);
  const updateAppearance = useSettingsStore((state) => state.updateAppearance);
  const isLightTheme = themeMode === "light";
  const isMaximized = useWindowMaximized("ProjectPicker");
  const toggleThemeMode = () => {
    const nextMode = isLightTheme ? "dark" : "light";
    updateAppearance({
      colorTheme: getRememberedColorTheme(nextMode, appearance.lastLightColorTheme, appearance.lastDarkColorTheme),
    });
  };

  return (
    <WindowMenuBar
      toolbar={
        <>
        {currentPath ? (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onGoEditor}
            className="mr-1 h-7 self-center px-3 text-muted-foreground hover:text-foreground"
          >
            {t("projectPicker.backToEditor")}
          </Button>
        ) : null}
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-lg"
          onClick={() => void openExternalUrl(APP_LINKS.repository)}
          className="self-center text-muted-foreground"
          tooltip={t("menubar.githubRepository")}
          aria-label={t("menubar.githubRepository")}
        >
          <VscGithub size={16} />
        </ToolbarIconButton>
        <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-lg"
        onClick={toggleThemeMode}
        className="self-center text-muted-foreground"
        tooltip={isLightTheme ? t("menubar.switchToDark") : t("menubar.switchToLight")}
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
      </ToolbarIconButton>
      <ToolbarIconButton
        type="button"
        variant="ghost"
        size="icon-lg"
        onClick={onOpenSettings}
        className="self-center text-muted-foreground"
        tooltip={t("menubar.settings")}
        aria-label={t("menubar.settings")}
      >
        <VscSettingsGear size={14} />
      </ToolbarIconButton>
        </>
      }
      windowActions={<WindowChromeControls isMaximized={isMaximized} />}
    >
      <div className="flex items-center gap-2 px-4 pointer-events-none self-center">
        <div className="flex h-5 w-5 items-center justify-center rounded bg-[var(--accent-color)]">
          <span className="text-xs font-black text-white">Y</span>
        </div>
        <div className="text-sm font-bold tracking-tight text-foreground">
          Yss<span className="text-[var(--accent-color)]">BI</span>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-2 self-center pl-2">
        <div className="w-[min(16rem,28vw)] min-w-[10rem]">
          <div className="flex h-7 items-center rounded-md border border-input bg-muted/50 shadow-inner">
            <span className="pl-2 text-muted-foreground">
              <VscSearch size={14} />
            </span>
            <Input
              value={filterQuery}
              onChange={(event) => onSetFilterQuery(event.target.value)}
              className="h-7 min-w-0 flex-1 border-0 bg-transparent px-2 py-1 text-sm text-foreground shadow-none placeholder:text-muted-foreground focus-visible:ring-0"
              placeholder={t("projectPicker.searchPlaceholder")}
            />
            <ToolbarIconButton
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={() => onSetFilterQuery("")}
              className="mr-1 text-muted-foreground hover:text-foreground/80"
              tooltip={t("projectPicker.clearSearch")}
              aria-label={t("projectPicker.clearSearch")}
            >
              <VscClose size={12} />
            </ToolbarIconButton>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1 text-sm text-muted-foreground">
          <Label htmlFor="project-sort" className="hidden shrink-0 text-sm text-muted-foreground lg:block">
            {t("projectPicker.sortLabel")}:
          </Label>
          <Select value={sortMode} onValueChange={(value) => onSetSortMode(value as SortMode)}>
            <SelectTrigger
              id="project-sort"
              size="sm"
              className="h-7 min-h-7 w-[8rem] rounded-md border border-border bg-card/80 px-2 text-sm font-medium text-foreground shadow-sm data-[size=sm]:h-7 data-[size=sm]:min-h-7 data-[size=sm]:py-0"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent
              position="popper"
              side="bottom"
              align="start"
              sideOffset={4}
              className="min-w-[var(--radix-select-trigger-width)] rounded-lg"
            >
              <SelectItem value="lastOpened">{t("projectPicker.sortRecent")}</SelectItem>
              <SelectItem value="name">{t("projectPicker.sortName")}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </WindowMenuBar>
  );
}

function ProjectSettingsDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const language = useSettingsStore((state) => state.appearance.language);
  const updateAppearance = useSettingsStore((state) => state.updateAppearance);

  const languageOptions = [
    { label: t("language.zhCN"), value: "zh-CN" },
    { label: t("language.enUS"), value: "en-US" },
  ];

  const handleLanguageChange = (value: string) => {
    const nextLanguage = value as AppLanguage;
    updateAppearance({ language: nextLanguage });
    void i18n.changeLanguage(nextLanguage);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(420px,92vw)]">
        <DialogHeader>
          <DialogTitle>{t("menubar.settings")}</DialogTitle>
          <DialogDescription>{t("projectPicker.settingsDescription")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 px-6 pb-5">
          <div className="space-y-1.5">
            <Label htmlFor="project-picker-language" className="text-sm font-medium text-foreground">
              {t("settings.labels.language")}
            </Label>
            <p className="text-xs leading-relaxed text-muted-foreground">
              {t("settings.descriptions.language")}
            </p>
          </div>
          <Select value={language} onValueChange={handleLanguageChange}>
            <SelectTrigger id="project-picker-language" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {languageOptions.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <DialogFooter>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {t("common.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function ProjectPickerScreen() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  // 主窗口几何状态：恢复尺寸/位置/最大化，并在关闭时持久化
  usePersistedWindow("main");
  const {
    busy,
    currentProjectId,
    projects,
    createProject,
    importProjectFromDisk,
    openRecentProject,
    scanProjectsFromFolder,
    cleanupInvalidProjects,
    removeProject,
    deleteProjectFiles,
    toggleFavorite,
  } = useProjectPicker();
  const [selectedId, setSelectedId] = useState<string | null>(currentProjectId);
  const [filterQuery, setFilterQuery] = useState("");
  const [sortMode, setSortMode] = useState<SortMode>("lastOpened");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [newProjectOpen, setNewProjectOpen] = useState(false);
  const [deleteConfirmProject, setDeleteConfirmProject] = useState<ManagedProject | null>(null);

  const filtered = useMemo(
    () => sortAndFilter(projects, filterQuery, sortMode),
    [projects, filterQuery, sortMode],
  );
  const selected = selectedId ? projects.find((project) => project.id === selectedId) : undefined;
  const isBusy = busy !== "idle";

  const {
    contextMenu,
    openContextMenu,
    closeContextMenu,
  } = usePositionedContextMenu<ProjectPickerContextMenuTarget>();

  const openListContextMenu = useCallback((event: React.MouseEvent) => {
    openContextMenu(event, { kind: "list" });
  }, [openContextMenu]);

  const handleListAreaClick = useCallback((event: React.MouseEvent) => {
    if ((event.target as HTMLElement).closest("[data-project-picker-item]")) {
      return;
    }
    setSelectedId(null);
  }, []);

  const revealInExplorer = useCallback(async (projectPath: string) => {
    try {
      await ProjectService.revealProjectPath(projectPath);
    } catch (error) {
      uiStore.showToast(
        t("contextMenu.sidebar.revealInExplorerFailed", {
          error: formatErrorMessage(error, "Unknown error"),
        }),
        "error",
      );
    }
  }, [t]);

  const contextMenuSections = useMemo(
    () => buildProjectPickerContextMenuSections(contextMenu, {
      openProject: (path) => void openRecentProject(path),
      toggleFavorite,
      removeProject,
      requestDeleteProjectFiles: setDeleteConfirmProject,
      revealInExplorer,
      newProject: () => setNewProjectOpen(true),
      importProject: () => void importProjectFromDisk(),
      scanProjects: () => void scanProjectsFromFolder(),
      cleanupProjects: () => void cleanupInvalidProjects(),
      isBusy,
    }, t),
    [
      contextMenu,
      cleanupInvalidProjects,
      importProjectFromDisk,
      isBusy,
      openRecentProject,
      removeProject,
      revealInExplorer,
      scanProjectsFromFolder,
      t,
      toggleFavorite,
    ],
  );

  const handleConfirmDeleteProject = useCallback(async (project: ManagedProject) => {
    await deleteProjectFiles(project.id);
    setSelectedId((current) => (current === project.id ? null : current));
  }, [deleteProjectFiles]);

  useEffect(() => {
    if (currentProjectId) {
      setSelectedId(currentProjectId);
    }
  }, [currentProjectId]);

  useEffect(() => {
    if (selectedId && !projects.some((project) => project.id === selectedId)) {
      setSelectedId(null);
    }
  }, [projects, selectedId]);

  return (
    <div className="flex h-screen min-h-0 w-full min-w-0 flex-col overflow-hidden bg-background text-foreground">
      <TitleBar
        filterQuery={filterQuery}
        sortMode={sortMode}
        onGoEditor={() => navigate("/editor")}
        onOpenSettings={() => setSettingsOpen(true)}
        onSetFilterQuery={setFilterQuery}
        onSetSortMode={setSortMode}
      />
      <NewProjectModal
        open={newProjectOpen}
        onOpenChange={setNewProjectOpen}
        onCreate={createProject}
      />
      <ProjectSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
      <DeleteProjectConfirmDialog
        project={deleteConfirmProject}
        onOpenChange={(open) => {
          if (!open) setDeleteConfirmProject(null);
        }}
        onConfirm={handleConfirmDeleteProject}
      />

      <div className="flex min-h-0 flex-1">
        <div
          className="flex min-h-0 min-w-0 flex-1 flex-col bg-background"
          onContextMenu={openListContextMenu}
          onClick={handleListAreaClick}
        >
          {filtered.length === 0 ? (
            <div className="flex h-full min-h-[12rem] flex-col items-center justify-center gap-2 px-6 text-center text-sm text-muted-foreground">
              <VscProject size={42} className="opacity-35" />
              {projects.length === 0 ? t("projectPicker.emptyTitle") : t("projectPicker.noMatchesTitle")}
              <p className="max-w-sm text-xs">{projects.length === 0 ? t("projectPicker.emptyDescription") : t("projectPicker.noMatchesDescription")}</p>
            </div>
          ) : (
            <OverlayScrollbar className="flex-1">
              <div className="min-h-full">
              <ul className="divide-y divide-border/60">
                {filtered.map((project) => {
                  const isSelected = selectedId === project.id;
                  const isFavorite = Boolean(project.isFavorite);
                  return (
                    <li key={project.id}>
                      <div
                        role="button"
                        tabIndex={0}
                        data-project-picker-item
                        onClick={() => setSelectedId(project.id)}
                        onDoubleClick={() => void openRecentProject(project.path)}
                        onContextMenu={(event) => {
                          setSelectedId(project.id);
                          openContextMenu(event, { kind: "project", project });
                        }}
                        onKeyDown={(event) => {
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault();
                            setSelectedId(project.id);
                          }
                        }}
                        className={cn(
                          "flex w-full cursor-pointer items-center gap-3 px-3 py-3 text-left transition",
                          isSelected ? "bg-primary/15" : "hover:bg-muted/50",
                        )}
                      >
                        <ToolbarIconButton
                          type="button"
                          variant="ghost"
                          size="icon-sm"
                          onClick={(event) => {
                            event.stopPropagation();
                            toggleFavorite(project.id);
                          }}
                          className="shrink-0 rounded-md text-muted-foreground hover:bg-muted hover:text-amber-600 dark:hover:text-amber-300"
                          tooltip={isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite")}
                          aria-label={isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite")}
                        >
                          {isFavorite ? (
                            <VscStarFull size={18} className="text-amber-400" />
                          ) : (
                            <VscStarEmpty size={18} />
                          )}
                        </ToolbarIconButton>
                        <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-xl bg-primary/15 shadow-inner ring-1 ring-border">
                          <VscProject size={32} className="text-primary" />
                        </div>
                        <div className="flex min-w-0 flex-1 flex-col gap-2">
                          <div className="flex min-w-0 items-start gap-2">
                            <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
                              <span className="min-w-0 truncate text-[15px] font-semibold leading-snug tracking-tight text-foreground">
                                {project.name}
                              </span>
                              {currentProjectId === project.id ? (
                                <span className="rounded-full border border-[var(--accent-color)]/35 bg-[var(--accent-color)]/10 px-2 py-0.5 text-[10px] font-medium text-[var(--accent-color)]">
                                  {t("projectPicker.currentBadge")}
                                </span>
                              ) : null}
                            </div>
                          </div>
                          <div className="flex min-w-0 items-center gap-2">
                            <div className="flex min-w-0 flex-1 items-center gap-1.5 text-[12px] text-muted-foreground">
                              <VscFolder className="shrink-0 opacity-70" size={14} />
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <span className="truncate font-mono leading-snug">{project.path}</span>
                                </TooltipTrigger>
                                <TooltipContent side="bottom" className="max-w-md break-all font-mono text-xs">
                                  {project.path}
                                </TooltipContent>
                              </Tooltip>
                            </div>
                            <span className="max-w-[min(260px,42%)] shrink-0 pl-1 text-right text-[12px] tabular-nums leading-snug text-muted-foreground/95">
                              {formatStamp(project.lastOpenedAt)}
                            </span>
                          </div>
                        </div>
                      </div>
                    </li>
                  );
                })}
              </ul>
              </div>
            </OverlayScrollbar>
          )}
        </div>

        <Card className={cn(APP_CARD_OUTER_CLASS, "flex w-[220px] shrink-0 flex-col gap-0 rounded-none border-0 border-l border-border bg-card py-0 shadow-none ring-0 sm:w-[240px]")}>
          <CardContent className="flex flex-col gap-2 p-2">
            <SidebarBtn
              primary
              disabled={isBusy}
              onClick={() => setNewProjectOpen(true)}
            >
              <span className="flex w-full min-w-0 items-center gap-2">
                <VscNewFile className="shrink-0 opacity-90" size={14} />
                <span className="min-w-0 flex-1 text-center">
                  {busy === "new" ? t("projectPicker.creating") : t("projectPicker.newProject")}
                </span>
              </span>
            </SidebarBtn>
            <SidebarBtn disabled={isBusy} onClick={() => void importProjectFromDisk()}>
              <span className="flex w-full min-w-0 items-center gap-2">
                <VscFolderOpened className="shrink-0 opacity-90" size={14} />
                <span className="min-w-0 flex-1 text-center">
                  {busy === "import" ? t("projectPicker.importing") : t("projectPicker.importProject")}
                </span>
              </span>
            </SidebarBtn>
            <SidebarBtn disabled={isBusy} onClick={() => void scanProjectsFromFolder()}>
              <span className="flex w-full min-w-0 items-center gap-2">
                <VscRefresh className="shrink-0 opacity-90" size={14} />
                <span className="min-w-0 flex-1 text-center">
                  {busy === "scan" ? t("projectPicker.scanning") : t("projectPicker.scanProjects")}
                </span>
              </span>
            </SidebarBtn>
            <div className="my-1 border-t border-border/60" />
            <SidebarBtn
              disabled={!selected || isBusy}
              onClick={() => selected && void openRecentProject(selected.path)}
            >
              <span className="flex w-full min-w-0 items-center gap-2">
                <VscDebugStart className="shrink-0 opacity-90" size={14} />
                <span className="min-w-0 flex-1 text-center">{t("projectPicker.enter")}</span>
              </span>
            </SidebarBtn>
            <SidebarBtn disabled={!selected} onClick={() => selected && toggleFavorite(selected.id)}>
              <span className="flex w-full min-w-0 items-center gap-2">
                {selected?.isFavorite ? (
                  <VscStarFull className="shrink-0 opacity-90" size={14} />
                ) : (
                  <VscStarEmpty className="shrink-0 opacity-80" size={14} />
                )}
                <span className="min-w-0 flex-1 text-center">
                  {selected?.isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite")}
                </span>
              </span>
            </SidebarBtn>
            <div className="my-1 border-t border-border/60" />
            <SidebarBtn disabled={isBusy} onClick={() => void cleanupInvalidProjects()}>
              <span className="flex w-full min-w-0 items-center gap-2">
                <VscClearAll className="shrink-0 opacity-90" size={14} />
                <span className="min-w-0 flex-1 text-center">
                  {busy === "cleanup" ? t("projectPicker.cleaningUp") : t("projectPicker.cleanupProjects")}
                </span>
              </span>
            </SidebarBtn>
            <SidebarBtn danger disabled={!selected} onClick={() => selected && removeProject(selected.id)}>
              <span className="flex w-full min-w-0 items-center gap-2">
                <VscTrash className="shrink-0 opacity-90" size={14} />
                <span className="min-w-0 flex-1 text-center">{t("projectPicker.removeFromList")}</span>
              </span>
            </SidebarBtn>
            <SidebarBtn
              danger
              disabled={!selected || isBusy}
              onClick={() => selected && setDeleteConfirmProject(selected)}
            >
              <span className="flex w-full min-w-0 items-center gap-2">
                <VscWarning className="shrink-0 opacity-90" size={14} />
                <span className="min-w-0 flex-1 text-center">{t("projectPicker.deleteProjectFiles")}</span>
              </span>
            </SidebarBtn>
          </CardContent>
        </Card>
      </div>
      {contextMenu && (
        <ContextMenu
          position={{ x: contextMenu.x, y: contextMenu.y }}
          sections={contextMenuSections}
          onClose={closeContextMenu}
        />
      )}
    </div>
  );
}
