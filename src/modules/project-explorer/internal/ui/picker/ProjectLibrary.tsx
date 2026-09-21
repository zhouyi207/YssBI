import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import {
  VscClose,
  VscSearch,
  VscFolder,
  VscProject,
  VscStarEmpty,
  VscStarFull,
} from "react-icons/vsc";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";
import type { ManagedProject } from "@/features/application/project";
import { cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { formatProjectStamp, type ProjectSortMode } from "./projectPickerViewUtils";

interface ProjectLibraryProps {
  filterQuery: string;
  sortMode: ProjectSortMode;
  onSetFilterQuery: (value: string) => void;
  onSetSortMode: (value: ProjectSortMode) => void;
  projects: ManagedProject[];
  filteredProjects: ManagedProject[];
  selectedId: string | null;
  onSelectProject: (id: string | null) => void;
  onOpenProject: (path: string) => void;
  onToggleFavorite: (id: string) => void;
  onListContextMenu: (event: MouseEvent) => void;
  onProjectContextMenu: (event: MouseEvent, project: ManagedProject) => void;
}

export function ProjectLibrary({
  filterQuery,
  sortMode,
  onSetFilterQuery,
  onSetSortMode,
  projects,
  filteredProjects,
  selectedId,
  onSelectProject,
  onOpenProject,
  onToggleFavorite,
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
    <>
      <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-border bg-muted/30 px-3 py-2">
        <div className="min-w-[10rem] flex-1">
          <div className="flex h-7 items-center rounded-md border border-input bg-muted/50 shadow-inner">
            <span className="pl-2 text-muted-foreground">
              <VscSearch size={14} />
            </span>
            <Input
              value={filterQuery}
              onChange={(event) => onSetFilterQuery(event.target.value)}
              className="h-7 min-w-0 flex-1 border-0 bg-transparent px-2 py-1 text-sm text-foreground shadow-none placeholder:text-muted-foreground focus-visible:ring-0"
              placeholder={t("projectPicker.searchPlaceholder")}
              aria-label={t("projectPicker.searchPlaceholder")}
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
          <Label htmlFor="project-sort" className="shrink-0 text-xs text-muted-foreground">
            {t("projectPicker.sortLabel")}:
          </Label>
          <Select
            value={sortMode}
            onValueChange={(value) => onSetSortMode(value as ProjectSortMode)}
          >
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
      <div
        className="flex min-h-0 min-w-0 flex-1 flex-col bg-background"
        onContextMenu={onListContextMenu}
        onClick={handleListAreaClick}
      >
        {filteredProjects.length === 0 ? (
          <Empty className="h-full min-h-[12rem] rounded-none px-6">
            <EmptyHeader>
              <EmptyMedia variant="icon" className="size-12 text-muted-foreground">
                <VscProject className="size-6" />
              </EmptyMedia>
              <EmptyTitle>
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
          </Empty>
        ) : (
          <ScrollArea className="flex-1">
            <div className="min-h-full">
              <ul className="divide-y divide-border/60">
                {filteredProjects.map((project) => {
                  const isSelected = selectedId === project.id;
                  const isFavorite = Boolean(project.isFavorite);
                  return (
                    <li key={project.id}>
                      <div
                        role="button"
                        tabIndex={0}
                        data-project-picker-item
                        onClick={() => onSelectProject(project.id)}
                        onDoubleClick={() => onOpenProject(project.path)}
                        onContextMenu={(event) => {
                          onSelectProject(project.id);
                          onProjectContextMenu(event, project);
                        }}
                        onKeyDown={(event) => {
                          if (event.target !== event.currentTarget) return;
                          if (event.key === "Enter" || event.key === " ") {
                            event.preventDefault();
                            onSelectProject(project.id);
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
                            onToggleFavorite(project.id);
                          }}
                          className="shrink-0 rounded-md text-muted-foreground hover:bg-muted hover:text-amber-600 dark:hover:text-amber-300"
                          tooltip={
                            isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite")
                          }
                          aria-label={
                            isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite")
                          }
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
                            </div>
                          </div>
                          <div className="flex min-w-0 items-center gap-2">
                            <div className="flex min-w-0 flex-1 items-center gap-1.5 text-[12px] text-muted-foreground">
                              <VscFolder className="shrink-0 opacity-70" size={14} />
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <span className="truncate font-mono leading-snug">
                                    {project.path}
                                  </span>
                                </TooltipTrigger>
                                <TooltipContent
                                  side="bottom"
                                  className="max-w-md break-all font-mono text-xs"
                                >
                                  {project.path}
                                </TooltipContent>
                              </Tooltip>
                            </div>
                            <span className="max-w-[min(260px,42%)] shrink-0 pl-1 text-right text-[12px] tabular-nums leading-snug text-muted-foreground/95">
                              {formatProjectStamp(project.lastOpenedAt)}
                            </span>
                          </div>
                        </div>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </div>
          </ScrollArea>
        )}
      </div>
    </>
  );
}
