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
    <Dialog
      open={project != null}
      onOpenChange={(open) => {
        if (!busy) onOpenChange(open);
      }}
    >
      <DialogContent
        onInteractOutside={(event) => {
          if (busy) event.preventDefault();
        }}
        onEscapeKeyDown={(event) => {
          if (busy) event.preventDefault();
        }}
        className="max-w-md border-border bg-card text-card-foreground ring-border sm:max-w-md"
      >
        <DialogHeader>
          <DialogTitle>{t("projectPicker.deleteProjectConfirm.title")}</DialogTitle>
        </DialogHeader>

        <div className="px-6 pb-5">
          <DialogDescription className="text-[13px] leading-relaxed text-muted-foreground">
            {t("projectPicker.deleteProjectConfirm.description", { name: project?.name ?? "" })}
          </DialogDescription>
        </div>

        <DialogFooter className="gap-2 sm:justify-end">
          <Button
            type="button"
            variant="outline"
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
};
