import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  VscClearAll,
  VscDebugStart,
  VscFolder,
  VscFolderOpened,
  VscProject,
  VscStarEmpty,
  VscStarFull,
  VscTrash,
  VscWarning,
} from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { ManagedProject } from "@/features/application/project";
import { cn } from "@/lib/utils";
import { ProjectFlowGraphic } from "./ProjectFlowGraphic";
import { formatProjectStamp } from "./projectPickerViewUtils";

function ActionButton({
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
        "h-8 w-full min-w-0 justify-start gap-2 rounded-md px-2.5 text-xs font-medium",
        primary &&
          "bg-[var(--accent-color)] text-primary-foreground hover:bg-[var(--accent-color-hover)]",
        !primary &&
          !danger &&
          "border-transparent bg-transparent text-foreground hover:bg-[var(--interactive-hover)]",
        danger && "border-transparent bg-transparent",
      )}
    >
      {children}
    </Button>
  );
}

interface ProjectPickerActionPanelProps {
  selected?: ManagedProject;
  currentProjectId: string | null;
  isBusy: boolean;
  cleaningUp: boolean;
  onOpenProject: (path: string) => void;
  onRevealProject: (path: string) => void;
  onToggleFavorite: (id: string) => void;
  onRemoveProject: (id: string) => void;
  onDeleteProject: (project: ManagedProject) => void;
  onCleanupProjects: () => void;
}

export function ProjectPickerActionPanel({
  selected,
  currentProjectId,
  isBusy,
  cleaningUp,
  onOpenProject,
  onRevealProject,
  onToggleFavorite,
  onRemoveProject,
  onDeleteProject,
  onCleanupProjects,
}: ProjectPickerActionPanelProps) {
  const { t } = useTranslation();

  return (
    <aside className="flex w-[284px] shrink-0 flex-col border-l border-[var(--strong-border)] bg-[var(--sidebar-bg)] max-[920px]:w-[244px] max-[760px]:hidden">
      <div className="flex h-[var(--titlebar-height)] shrink-0 items-center border-b border-[var(--strong-border)] px-4">
        <span className="font-heading text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
          {t("projectPicker.actionsTitle")}
        </span>
      </div>
      <ScrollArea className="flex-1">
        {selected ? (
          <div className="flex flex-col gap-4 p-4">
            <div className="flex items-start gap-3">
              <div className="flex size-10 shrink-0 items-center justify-center rounded-lg border border-[var(--accent-color)]/25 bg-[var(--accent-color)]/10 text-[var(--accent-color)]">
                <VscProject size={21} />
              </div>
              <div className="min-w-0 flex-1 pt-0.5">
                <div className="flex min-w-0 items-center gap-2">
                  <h2 className="truncate font-heading text-sm font-semibold tracking-[-0.02em] text-foreground">
                    {selected.name}
                  </h2>
                  {currentProjectId === selected.id ? (
                    <span className="size-1.5 shrink-0 rounded-full bg-[var(--accent-color)] shadow-[0_0_0_3px_color-mix(in_srgb,var(--accent-color)_14%,transparent)]" />
                  ) : null}
                </div>
                <p className="mt-1 font-mono text-[10px] tabular-nums text-muted-foreground">
                  {formatProjectStamp(selected.lastOpenedAt)}
                </p>
              </div>
            </div>

            <div className="rounded-md border border-[var(--strong-border)] bg-[var(--surface-sunken)]/65 p-2.5">
              <div className="flex items-start gap-2 text-muted-foreground">
                <VscFolder className="mt-0.5 shrink-0" size={12} />
                <span className="break-all font-mono text-[10px] leading-4">{selected.path}</span>
              </div>
            </div>

            <div className="flex flex-col gap-1">
              <ActionButton primary disabled={isBusy} onClick={() => onOpenProject(selected.path)}>
                <VscDebugStart size={14} />
                <span>{t("projectPicker.enter")}</span>
              </ActionButton>
              <ActionButton disabled={isBusy} onClick={() => onRevealProject(selected.path)}>
                <VscFolderOpened size={14} />
                <span>{t("contextMenu.sidebar.revealInExplorer")}</span>
              </ActionButton>
              <ActionButton disabled={isBusy} onClick={() => onToggleFavorite(selected.id)}>
                {selected.isFavorite ? <VscStarFull size={14} /> : <VscStarEmpty size={14} />}
                <span>
                  {selected.isFavorite
                    ? t("projectPicker.unfavorite")
                    : t("projectPicker.favorite")}
                </span>
              </ActionButton>
            </div>

            <div className="border-t border-[var(--strong-border)] pt-3">
              <ActionButton danger disabled={isBusy} onClick={() => onRemoveProject(selected.id)}>
                <VscTrash size={14} />
                <span>{t("projectPicker.removeFromList")}</span>
              </ActionButton>
              <ActionButton danger disabled={isBusy} onClick={() => onDeleteProject(selected)}>
                <VscWarning size={14} />
                <span>{t("projectPicker.deleteProjectFiles")}</span>
              </ActionButton>
            </div>
          </div>
        ) : (
          <div className="flex min-h-[360px] flex-col items-center justify-center px-6 text-center">
            <div className="mb-5 w-full overflow-hidden rounded-lg border border-[var(--strong-border)] bg-[var(--surface-sunken)]/55 p-2">
              <ProjectFlowGraphic className="h-auto w-full opacity-60" />
            </div>
            <p className="font-heading text-sm font-semibold text-foreground">
              {t("projectPicker.actionsTitle")}
            </p>
            <p className="mt-1.5 max-w-[210px] text-xs leading-5 text-muted-foreground">
              {t("projectPicker.noSelection")}
            </p>
          </div>
        )}
      </ScrollArea>
      <div className="shrink-0 border-t border-[var(--strong-border)] p-3">
        <ActionButton disabled={isBusy} onClick={onCleanupProjects}>
          <VscClearAll size={14} />
          <span>
            {cleaningUp ? t("projectPicker.cleaningUp") : t("projectPicker.cleanupProjects")}
          </span>
        </ActionButton>
      </div>
    </aside>
  );
}
