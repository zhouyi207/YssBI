import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscError, VscWarning } from "react-icons/vsc";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  projectPickerErrorPresentation,
  type ManagedProject,
  type ProjectPickerLifecycleActionOutcome,
} from "@/features/application/project";
import {
  ProjectPickerErrorDetails,
  ProjectPickerRecoveryDetails,
  ProjectPickerStaleDetails,
} from "./ProjectPickerFeedbackDetails";

interface DeleteProjectConfirmDialogProps {
  project: ManagedProject | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: (project: ManagedProject) => Promise<ProjectPickerLifecycleActionOutcome>;
}

type DeleteProjectIssue = Exclude<ProjectPickerLifecycleActionOutcome, { status: "committed" }>;

function DeleteProjectIssueAlert({ issue }: { issue: DeleteProjectIssue }) {
  const { t } = useTranslation();

  if (issue.status === "failed") {
    return (
      <Alert variant="destructive">
        <VscError aria-hidden="true" />
        <AlertTitle>{t("projectPicker.deleteProjectConfirm.failed")}</AlertTitle>
        <AlertDescription>
          <ProjectPickerErrorDetails error={issue.error} />
        </AlertDescription>
      </Alert>
    );
  }

  if (issue.status === "recovery") {
    return (
      <Alert variant="warning">
        <VscWarning aria-hidden="true" />
        <AlertTitle>
          {t("notifications.projectPicker.deleteRecovery", {
            outcome: issue.recovery.action,
          })}
        </AlertTitle>
        <AlertDescription>
          <ProjectPickerRecoveryDetails recovery={issue.recovery} />
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <Alert variant="warning">
      <VscWarning aria-hidden="true" />
      <AlertTitle>
        {t("projectPicker.issues.staleTitle", { defaultValue: t("common.error") })}
      </AlertTitle>
      <AlertDescription>
        <ProjectPickerStaleDetails />
      </AlertDescription>
    </Alert>
  );
}

export function DeleteProjectConfirmDialog({
  project,
  onOpenChange,
  onConfirm,
}: DeleteProjectConfirmDialogProps) {
  return (
    <Dialog open={project != null} onOpenChange={onOpenChange}>
      {project ? (
        <DeleteProjectConfirmation
          key={project.id}
          project={project}
          onOpenChange={onOpenChange}
          onConfirm={onConfirm}
        />
      ) : null}
    </Dialog>
  );
}

function DeleteProjectConfirmation({
  project,
  onOpenChange,
  onConfirm,
}: DeleteProjectConfirmDialogProps & { project: ManagedProject }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [issue, setIssue] = useState<DeleteProjectIssue | null>(null);
  const mounted = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const handleConfirm = async () => {
    if (busy) return;
    setIssue(null);
    setBusy(true);
    try {
      const outcome = await onConfirm(project);
      if (!mounted.current) return;
      if (outcome.status === "committed") {
        onOpenChange(false);
      } else {
        setIssue(outcome);
      }
    } catch (error) {
      if (!mounted.current) return;
      setIssue({ status: "failed", error: projectPickerErrorPresentation(error) });
    } finally {
      if (mounted.current) setBusy(false);
    }
  };

  return (
    <DialogContent className="max-w-md border-border bg-card text-card-foreground ring-border sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{t("projectPicker.deleteProjectConfirm.title")}</DialogTitle>
      </DialogHeader>

      <div className="space-y-3 px-6 pb-5">
        <DialogDescription className="text-[13px] leading-relaxed text-muted-foreground">
          {t("projectPicker.deleteProjectConfirm.description", { name: project.name })}
        </DialogDescription>
        {issue ? <DeleteProjectIssueAlert issue={issue} /> : null}
      </div>

      <DialogFooter className="gap-2 sm:justify-end">
        <Button type="button" variant="outline" disabled={busy} onClick={() => onOpenChange(false)}>
          {t("common.cancel")}
        </Button>
        <Button
          type="button"
          variant="destructive"
          disabled={busy || issue?.status === "recovery"}
          onClick={() => void handleConfirm()}
        >
          {busy
            ? t("projectPicker.deleteProjectConfirm.deleting")
            : t("projectPicker.deleteProjectConfirm.confirm")}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
