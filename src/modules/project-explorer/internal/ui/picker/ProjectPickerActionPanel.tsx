import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  VscClearAll,
  VscDebugStart,
  VscFolderOpened,
  VscNewFile,
  VscRefresh,
  VscStarEmpty,
  VscStarFull,
  VscTrash,
  VscWarning,
} from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import type { ManagedProject } from "@/features/application/project";
import { cn } from "@/lib/utils";

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
        "h-auto w-full min-w-0 justify-start rounded px-3 py-2 text-[12px] font-medium",
        !primary && !danger && "border-border bg-muted/70 text-foreground hover:bg-muted",
      )}
    >
      {children}
    </Button>
  );
}

interface ProjectPickerActionPanelProps {
  selected?: ManagedProject;
  isBusy: boolean;
  cleaningUp: boolean;
  creating: boolean;
  importing: boolean;
  scanning: boolean;
  onNewProject: () => void;
  onImportProject: () => void;
  onScanProjects: () => void;
  onOpenProject: (path: string) => void;
  onToggleFavorite: (id: string) => void;
  onRemoveProject: (id: string) => void;
  onDeleteProject: (project: ManagedProject) => void;
  onCleanupProjects: () => void;
}

export function ProjectPickerActionPanel({
  selected,
  isBusy,
  cleaningUp,
  creating,
  importing,
  scanning,
  onNewProject,
  onImportProject,
  onScanProjects,
  onOpenProject,
  onToggleFavorite,
  onRemoveProject,
  onDeleteProject,
  onCleanupProjects,
}: ProjectPickerActionPanelProps) {
  const { t } = useTranslation();

  return (
    <Card className="flex min-h-0 w-[220px] shrink-0 flex-col gap-0 overflow-hidden rounded-none border-0 border-l border-border bg-card py-0 shadow-none ring-0 sm:w-[240px]">
      <CardContent className="flex min-h-0 flex-col gap-2 overflow-y-auto p-2">
        <ActionButton primary disabled={isBusy} onClick={onNewProject}>
          <span className="flex w-full min-w-0 items-center gap-2">
            <VscNewFile className="shrink-0 opacity-90" size={14} />
            <span className="min-w-0 flex-1 text-center">
              {creating ? t("projectPicker.creating") : t("projectPicker.newProject")}
            </span>
          </span>
        </ActionButton>
        <ActionButton disabled={isBusy} onClick={onImportProject}>
          <span className="flex w-full min-w-0 items-center gap-2">
            <VscFolderOpened className="shrink-0 opacity-90" size={14} />
            <span className="min-w-0 flex-1 text-center">
              {importing ? t("projectPicker.importing") : t("projectPicker.importProject")}
            </span>
          </span>
        </ActionButton>
        <ActionButton disabled={isBusy} onClick={onScanProjects}>
          <span className="flex w-full min-w-0 items-center gap-2">
            <VscRefresh className="shrink-0 opacity-90" size={14} />
            <span className="min-w-0 flex-1 text-center">
              {scanning ? t("projectPicker.scanning") : t("projectPicker.scanProjects")}
            </span>
          </span>
        </ActionButton>
        <div className="my-1 border-t border-border/60" />
        <ActionButton
          disabled={!selected || isBusy}
          onClick={() => selected && onOpenProject(selected.path)}
        >
          <span className="flex w-full min-w-0 items-center gap-2">
            <VscDebugStart className="shrink-0 opacity-90" size={14} />
            <span className="min-w-0 flex-1 text-center">{t("projectPicker.enter")}</span>
          </span>
        </ActionButton>
        <ActionButton
          disabled={!selected || isBusy}
          onClick={() => selected && onToggleFavorite(selected.id)}
        >
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
        </ActionButton>
      </CardContent>
      <CardContent className="mt-auto flex shrink-0 flex-col gap-2 border-t border-border/60 p-2">
        <ActionButton disabled={isBusy} onClick={onCleanupProjects}>
          <span className="flex w-full min-w-0 items-center gap-2">
            <VscClearAll className="shrink-0 opacity-90" size={14} />
            <span className="min-w-0 flex-1 text-center">
              {cleaningUp ? t("projectPicker.cleaningUp") : t("projectPicker.cleanupProjects")}
            </span>
          </span>
        </ActionButton>
        <ActionButton
          danger
          disabled={!selected || isBusy}
          onClick={() => selected && onRemoveProject(selected.id)}
        >
          <span className="flex w-full min-w-0 items-center gap-2">
            <VscTrash className="shrink-0 opacity-90" size={14} />
            <span className="min-w-0 flex-1 text-center">{t("projectPicker.removeFromList")}</span>
          </span>
        </ActionButton>
        <ActionButton
          danger
          disabled={!selected || isBusy}
          onClick={() => selected && onDeleteProject(selected)}
        >
          <span className="flex w-full min-w-0 items-center gap-2">
            <VscWarning className="shrink-0 opacity-90" size={14} />
            <span className="min-w-0 flex-1 text-center">
              {t("projectPicker.deleteProjectFiles")}
            </span>
          </span>
        </ActionButton>
      </CardContent>
    </Card>
  );
}
