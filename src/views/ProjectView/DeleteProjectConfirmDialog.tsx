import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { ManagedProject } from "@/features/application/project";

interface DeleteProjectConfirmDialogProps {
  project: ManagedProject | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: (project: ManagedProject) => Promise<void>;
}

export function DeleteProjectConfirmDialog({
  project,
  onOpenChange,
  onConfirm,
}: DeleteProjectConfirmDialogProps) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);

  const handleConfirm = async () => {
    if (!project || busy) return;
    setBusy(true);
    try {
      await onConfirm(project);
      onOpenChange(false);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={project != null} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("projectPicker.deleteProjectConfirm.title")}</DialogTitle>
          <DialogDescription>
            {t("projectPicker.deleteProjectConfirm.description", { name: project?.name ?? "" })}
          </DialogDescription>
        </DialogHeader>

        {project ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs leading-relaxed text-muted-foreground">
            <p className="mb-1 font-medium text-foreground">
              {t("projectPicker.deleteProjectConfirm.pathLabel")}
            </p>
            <p className="break-all font-mono">{project.path}</p>
          </div>
        ) : null}

        <DialogFooter>
          <Button
            type="button"
            variant="secondary"
            disabled={busy}
            onClick={() => onOpenChange(false)}
          >
            {t("common.cancel")}
          </Button>
          <Button
            type="button"
            variant="destructive"
            disabled={busy || !project}
            onClick={() => void handleConfirm()}
          >
            {busy
              ? t("projectPicker.deleteProjectConfirm.deleting")
              : t("projectPicker.deleteProjectConfirm.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
