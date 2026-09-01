import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscError, VscWarning } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import {
  getDefaultProjectParentDirectory,
  openProjectPathDialog,
  projectPickerErrorPresentation,
  type ProjectPickerErrorPresentation,
  type ProjectPickerLifecycleActionOutcome,
  type ProjectPickerRecoveryPresentation,
} from "@/features/application/project";
import { DEFAULT_PROJECT_NAME } from "@/shared/constants/defaultResourceNames";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";
import {
  ProjectPickerErrorDetails,
  ProjectPickerRecoveryDetails,
  ProjectPickerStaleDetails,
} from "./ProjectPickerFeedbackDetails";

function joinPath(parent: string, child: string) {
  const base = parent.replace(/[/\\]+$/, "");
  const leaf = child.replace(/^[/\\]+/, "");
  if (!base) return leaf;
  if (!leaf) return base;
  const separator =
    /^[a-zA-Z]:$/.test(base) || (base.includes("\\") && !base.includes("/")) ? "\\" : "/";
  return `${base}${separator}${leaf}`;
}

function parentDirectoryOf(fullPath: string): string {
  const trimmed = fullPath.trim().replace(/[/\\]+$/, "");
  if (!trimmed) return "";
  const index = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
  return index < 0 ? "" : trimmed.slice(0, index);
}

function sanitizeDirSegment(name: string) {
  return (
    (name.trim() || "untitled")
      .replace(/[\\/:*?"<>|]/g, "-")
      .replace(/\s+/g, " ")
      .trim() || "untitled"
  );
}

function lastPathSegment(path: string) {
  const trimmed = path.trim().replace(/[/\\]+$/, "");
  return trimmed.split(/[/\\]/u).filter(Boolean).pop() ?? "";
}

interface NewProjectModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreate: (name: string, path: string) => Promise<ProjectPickerLifecycleActionOutcome>;
}

type FieldErrors = {
  path: boolean;
  name: boolean;
};

type NewProjectIssue =
  | {
      kind: "failure";
      operation: "defaultPath" | "browse" | "create";
      error: ProjectPickerErrorPresentation;
    }
  | { kind: "recovery"; recovery: ProjectPickerRecoveryPresentation }
  | { kind: "stale" };

const emptyFieldErrors = (): FieldErrors => ({ path: false, name: false });

const FAILURE_TITLE_KEYS: Record<
  Extract<NewProjectIssue, { kind: "failure" }>["operation"],
  string
> = {
  defaultPath: "notifications.newProject.defaultPathFailed",
  browse: "notifications.newProject.browseFailed",
  create: "notifications.newProject.createFailed",
};

function NewProjectIssueAlert({ issue, name }: { issue: NewProjectIssue; name: string }) {
  const { t } = useTranslation();

  if (issue.kind === "failure") {
    const errorMessage = t(issue.error.messageKey, {
      defaultValue: t(issue.error.fallbackMessageKey),
    });
    return (
      <Alert variant="destructive">
        <VscError aria-hidden="true" />
        <AlertTitle>{t(FAILURE_TITLE_KEYS[issue.operation], { error: errorMessage })}</AlertTitle>
        <AlertDescription>
          <ProjectPickerErrorDetails error={issue.error} />
        </AlertDescription>
      </Alert>
    );
  }

  if (issue.kind === "recovery") {
    return (
      <Alert variant="warning">
        <VscWarning aria-hidden="true" />
        <AlertTitle>
          {t("notifications.projectPicker.createRecovery", {
            name,
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

export function NewProjectModal({ open: isOpen, onOpenChange, onCreate }: NewProjectModalProps) {
  const { t } = useTranslation();
  const [parentBase, setParentBase] = useState("");
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [pathAuto, setPathAuto] = useState(true);
  const [fieldErrors, setFieldErrors] = useState<FieldErrors>(emptyFieldErrors);
  const [issue, setIssue] = useState<NewProjectIssue | null>(null);
  const [busy, setBusy] = useState(false);

  function clearFieldErrors() {
    setFieldErrors(emptyFieldErrors());
  }

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    clearFieldErrors();
    setIssue(null);
    setName(DEFAULT_PROJECT_NAME);
    setPath("");
    setParentBase("");
    setPathAuto(true);

    (async () => {
      try {
        const parent = formatDisplayPath(await getDefaultProjectParentDirectory());
        if (cancelled) return;
        const defaultName = DEFAULT_PROJECT_NAME;
        setParentBase(parent);
        setName(defaultName);
        setPath(joinPath(parent, sanitizeDirSegment(defaultName)));
      } catch (error) {
        if (cancelled) return;
        setPathAuto(false);
        setIssue({
          kind: "failure",
          operation: "defaultPath",
          error: projectPickerErrorPresentation(error),
        });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  function updateName(nextName: string) {
    setName(nextName);
    clearFieldErrors();
    setIssue(null);
    if (!pathAuto) return;
    setPath(joinPath(parentBase, sanitizeDirSegment(nextName)));
  }

  async function browseParentDirectory() {
    clearFieldErrors();
    setIssue(null);
    try {
      const result = await openProjectPathDialog({
        directory: true,
        multiple: false,
        title: t("projectPicker.newProjectModal.browseTitle"),
        defaultPath: parentBase || undefined,
      });
      if (!result.ok) throw new Error(result.failure.code);
      const selected = result.value;
      if (!selected || Array.isArray(selected)) return;
      const parent = formatDisplayPath(selected);
      setParentBase(parent);
      setPathAuto(true);
      setPath(joinPath(parent, sanitizeDirSegment(name)));
    } catch (error) {
      setIssue({
        kind: "failure",
        operation: "browse",
        error: projectPickerErrorPresentation(error),
      });
    }
  }

  async function handleCreate() {
    clearFieldErrors();
    setIssue(null);
    setBusy(true);
    try {
      const outcome = await onCreate(name.trim(), path.trim());
      if (outcome.status === "committed") {
        onOpenChange(false);
      } else if (outcome.status === "failed") {
        setFieldErrors({ path: true, name: true });
        setIssue({ kind: "failure", operation: "create", error: outcome.error });
      } else if (outcome.status === "recovery") {
        setIssue({ kind: "recovery", recovery: outcome.recovery });
      } else {
        setIssue({ kind: "stale" });
      }
    } catch (error) {
      setFieldErrors({ path: true, name: true });
      setIssue({
        kind: "failure",
        operation: "create",
        error: projectPickerErrorPresentation(error),
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => {
        if (!busy) onOpenChange(nextOpen);
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
          <DialogTitle>{t("projectPicker.newProjectModal.title")}</DialogTitle>
        </DialogHeader>

        <div className="space-y-4 px-6 pb-5">
          {issue ? <NewProjectIssueAlert issue={issue} name={name.trim()} /> : null}
          <div className="space-y-1">
            <Label htmlFor="new-project-path" className="text-[12px] text-muted-foreground">
              {t("projectPicker.newProjectModal.pathLabel")}
            </Label>
            <div className="flex items-center gap-2">
              <Input
                id="new-project-path"
                value={path}
                aria-invalid={fieldErrors.path}
                onChange={(event) => {
                  const nextPath = event.target.value;
                  clearFieldErrors();
                  setIssue(null);
                  setPathAuto(false);
                  setPath(nextPath);
                  const nextName = lastPathSegment(nextPath);
                  if (nextName) setName(nextName);
                  setParentBase(parentDirectoryOf(nextPath));
                }}
                className={cn(
                  "min-w-0 flex-1 bg-muted/50 font-mono text-[12px] text-foreground",
                  fieldErrors.path ? "border-destructive" : "border-input",
                )}
                autoComplete="off"
                spellCheck={false}
              />
              <Button
                type="button"
                variant="outline"
                disabled={busy}
                onClick={() => void browseParentDirectory()}
                className="h-9 shrink-0 border-border bg-muted px-3 text-[12px] text-foreground hover:bg-muted/80"
              >
                {t("projectPicker.newProjectModal.browse")}
              </Button>
            </div>
          </div>

          <div className="space-y-1">
            <Label htmlFor="new-project-name" className="text-[12px] text-muted-foreground">
              {t("projectPicker.newProjectModal.nameLabel")}
            </Label>
            <Input
              id="new-project-name"
              value={name}
              aria-invalid={fieldErrors.name}
              onChange={(event) => updateName(event.target.value)}
              className={cn(
                "bg-muted/50 text-[13px] text-foreground",
                fieldErrors.name ? "border-destructive" : "border-input",
              )}
              autoComplete="off"
            />
          </div>
        </div>

        <DialogFooter className="gap-2 sm:justify-end">
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={busy}
          >
            {t("common.cancel")}
          </Button>
          <Button
            type="button"
            onClick={() => void handleCreate()}
            disabled={busy || issue?.kind === "recovery"}
          >
            {busy ? t("projectPicker.creating") : t("projectPicker.newProjectModal.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
