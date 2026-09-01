import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import {
  VscClose,
  VscDebugStart,
  VscFolder,
  VscFolderOpened,
  VscNewFile,
  VscProject,
  VscSearch,
  VscStarEmpty,
  VscStarFull,
} from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { ManagedProject } from "@/features/application/project";
import { cn } from "@/lib/utils";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";
import { formatProjectStamp, type ProjectSortMode } from "./projectPickerViewUtils";

interface ProjectLibraryProps {
  projects: ManagedProject[];
  filteredProjects: ManagedProject[];
  selectedId: string | null;
  currentProjectId: string | null;
  filterQuery: string;
  sortMode: ProjectSortMode;
  isBusy: boolean;
  onFilterQueryChange: (value: string) => void;
  onSortModeChange: (value: ProjectSortMode) => void;
  onSelectProject: (id: string | null) => void;
  onOpenProject: (path: string) => void;
  onToggleFavorite: (id: string) => void;
  onNewProject: () => void;
  onImportProject: () => void;
  onListContextMenu: (event: MouseEvent) => void;
  onProjectContextMenu: (event: MouseEvent, project: ManagedProject) => void;
}

export function ProjectLibrary({
  projects,
  filteredProjects,
  selectedId,
  currentProjectId,
  filterQuery,
  sortMode,
  isBusy,
  onFilterQueryChange,
  onSortModeChange,
  onSelectProject,
  onOpenProject,
  onToggleFavorite,
  onNewProject,
  onImportProject,
  onListContextMenu,
  onProjectContextMenu,
}: ProjectLibraryProps) {
  const { t } = useTranslation();

  const handleListAreaClick = (event: MouseEvent) => {
    if (!(event.target as HTMLElement).closest("[data-project-picker-item]")) {
      onSelectProject(null);
    }
  };

  return (
    <section className="flex min-h-0 flex-1 flex-col bg-background/72">
      <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-[var(--strong-border)] bg-[var(--panel-header-bg)]/85 px-5 py-3">
        <div className="mr-auto flex min-w-0 items-baseline gap-2">
          <h2 className="font-heading text-sm font-semibold tracking-[-0.02em] text-foreground">
            {t("projectPicker.title")}
          </h2>
          <span className="font-mono text-[10px] tabular-nums text-muted-foreground">
            {String(filteredProjects.length).padStart(2, "0")} /{" "}
            {String(projects.length).padStart(2, "0")}
          </span>
        </div>

        <div className="flex h-8 w-[min(20rem,38vw)] min-w-[11rem] items-center rounded-md border border-input bg-[var(--surface-raised)] shadow-sm focus-within:border-[var(--accent-color)] focus-within:ring-2 focus-within:ring-[var(--accent-color)]/20 max-[640px]:order-last max-[640px]:w-full">
          <span className="pl-2.5 text-muted-foreground">
            <VscSearch size={14} />
          </span>
          <Input
            value={filterQuery}
            onChange={(event) => onFilterQueryChange(event.target.value)}
            className="h-7 min-w-0 flex-1 border-0 bg-transparent px-2 py-1 text-xs shadow-none focus-visible:ring-0"
            placeholder={t("projectPicker.searchPlaceholder")}
          />
          {filterQuery ? (
            <ToolbarIconButton
              type="button"
              variant="ghost"
              size="icon-xs"
              onClick={() => onFilterQueryChange("")}
              className="mr-1 text-muted-foreground"
              tooltip={t("projectPicker.clearSearch")}
              aria-label={t("projectPicker.clearSearch")}
            >
              <VscClose size={12} />
            </ToolbarIconButton>
          ) : null}
        </div>

        <Label htmlFor="project-sort" className="sr-only">
          {t("projectPicker.sortLabel")}
        </Label>
        <Select
          value={sortMode}
          onValueChange={(value) => onSortModeChange(value as ProjectSortMode)}
        >
          <SelectTrigger
            id="project-sort"
            size="sm"
            aria-label={t("projectPicker.sortLabel")}
            className="h-8 min-h-8 w-[8.5rem] border-input bg-[var(--surface-raised)] px-2 text-xs shadow-sm data-[size=sm]:h-8 data-[size=sm]:min-h-8 data-[size=sm]:py-0"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent position="popper" side="bottom" align="end" sideOffset={4}>
            <SelectItem value="lastOpened">{t("projectPicker.sortRecent")}</SelectItem>
            <SelectItem value="name">{t("projectPicker.sortName")}</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div
        className="flex min-h-0 flex-1 flex-col"
        onContextMenu={onListContextMenu}
        onClick={handleListAreaClick}
      >
        {filteredProjects.length === 0 ? (
          <Empty className="h-full min-h-[12rem] rounded-none px-6">
            <EmptyHeader>
              <EmptyMedia
                variant="icon"
                className="size-12 border border-[var(--strong-border)] bg-[var(--surface-raised)] text-[var(--accent-color)] shadow-sm"
              >
                <VscProject className="size-6" />
              </EmptyMedia>
              <EmptyTitle className="font-heading text-base">
                {projects.length === 0
                  ? t("projectPicker.emptyTitle")
                  : t("projectPicker.noMatchesTitle")}
              </EmptyTitle>
              <EmptyDescription>
                {projects.length === 0
                  ? t("projectPicker.emptyDescription")
                  : t("projectPicker.noMatchesDescription")}
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent className="flex-row justify-center">
              {projects.length === 0 ? (
                <>
                  <Button type="button" onClick={onNewProject} disabled={isBusy}>
                    <VscNewFile />
                    {t("projectPicker.newProject")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={onImportProject}
                    disabled={isBusy}
                  >
                    <VscFolderOpened />
                    {t("projectPicker.importProject")}
                  </Button>
                </>
              ) : (
                <Button type="button" variant="outline" onClick={() => onFilterQueryChange("")}>
                  <VscClose />
                  {t("projectPicker.clearSearch")}
                </Button>
              )}
            </EmptyContent>
          </Empty>
        ) : (
          <ScrollArea className="flex-1">
            <ul className="grid min-h-full grid-cols-1 content-start gap-2 p-4 2xl:grid-cols-2">
              {filteredProjects.map((project) => {
                const isSelected = selectedId === project.id;
                const isFavorite = Boolean(project.isFavorite);
                return (
                  <li key={project.id} className="relative min-w-0">
                    <button
                      type="button"
                      data-project-picker-item
                      data-selected={isSelected ? "true" : "false"}
                      onClick={() => onSelectProject(project.id)}
                      onDoubleClick={() => onOpenProject(project.path)}
                      onContextMenu={(event) => onProjectContextMenu(event, project)}
                      className={cn(
                        "group relative flex min-h-[92px] w-full cursor-pointer items-center gap-3 rounded-lg border px-3.5 py-3 pr-11 text-left shadow-sm transition-[border-color,background-color,box-shadow,transform] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent-color)]/35",
                        isSelected
                          ? "border-[var(--accent-color)]/55 bg-[var(--accent-color)]/10 shadow-[0_8px_24px_color-mix(in_srgb,var(--accent-color)_10%,transparent)]"
                          : "border-[var(--strong-border)] bg-[var(--surface-raised)]/82 hover:-translate-y-px hover:border-[var(--accent-color)]/30 hover:bg-[var(--surface-raised)]",
                      )}
                    >
                      <div
                        className={cn(
                          "flex size-11 shrink-0 items-center justify-center rounded-lg border shadow-inner",
                          isSelected
                            ? "border-[var(--accent-color)]/25 bg-[var(--accent-color)]/12 text-[var(--accent-color)]"
                            : "border-[var(--strong-border)] bg-[var(--surface-sunken)] text-muted-foreground group-hover:text-[var(--accent-color)]",
                        )}
                      >
                        <VscProject size={23} />
                      </div>
                      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                        <div className="flex min-w-0 items-center gap-2">
                          <span className="min-w-0 truncate font-heading text-sm font-semibold tracking-[-0.02em] text-foreground">
                            {project.name}
                          </span>
                          {currentProjectId === project.id ? (
                            <span className="shrink-0 rounded-full border border-[var(--accent-color)]/30 bg-[var(--accent-color)]/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-[var(--accent-color)]">
                              {t("projectPicker.currentBadge")}
                            </span>
                          ) : null}
                        </div>
                        <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
                          <VscFolder className="shrink-0 opacity-70" size={12} />
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <span className="truncate font-mono">{project.path}</span>
                            </TooltipTrigger>
                            <TooltipContent
                              side="bottom"
                              className="max-w-md break-all font-mono text-xs"
                            >
                              {project.path}
                            </TooltipContent>
                          </Tooltip>
                        </div>
                        <span className="font-mono text-[10px] tabular-nums text-muted-foreground">
                          {formatProjectStamp(project.lastOpenedAt)}
                        </span>
                      </div>
                    </button>
                    <ToolbarIconButton
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={(event) => {
                        event.stopPropagation();
                        onToggleFavorite(project.id);
                      }}
                      className="absolute right-3 top-3 rounded-md text-muted-foreground hover:text-amber-600 dark:hover:text-amber-300"
                      tooltip={
                        isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite")
                      }
                      aria-label={
                        isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite")
                      }
                    >
                      {isFavorite ? (
                        <VscStarFull size={16} className="text-amber-400" />
                      ) : (
                        <VscStarEmpty size={16} />
                      )}
                    </ToolbarIconButton>
                    <ToolbarIconButton
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      onClick={(event) => {
                        event.stopPropagation();
                        onOpenProject(project.path);
                      }}
                      className="absolute bottom-3 right-3 hidden text-[var(--accent-color)] max-[760px]:inline-flex"
                      tooltip={t("projectPicker.enter")}
                      aria-label={t("projectPicker.enter")}
                    >
                      <VscDebugStart size={14} />
                    </ToolbarIconButton>
                  </li>
                );
              })}
            </ul>
          </ScrollArea>
        )}
      </div>
    </section>
  );
}
