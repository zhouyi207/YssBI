import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { DEFAULT_PROJECT_NAME } from "@/shared/constants/defaultResourceNames";
import { ProjectService } from "@/services/project/projectService";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { formatDisplayPath } from "@/shared/utils/formatDisplayPath";

function joinPath(parent: string, child: string) {
  const base = parent.replace(/[/\\]+$/, "");
  const leaf = child.replace(/^[/\\]+/, "");
  if (!base) return leaf;
  if (!leaf) return base;
  const separator = /^[a-zA-Z]:$/.test(base) || (base.includes("\\") && !base.includes("/")) ? "\\" : "/";
  return `${base}${separator}${leaf}`;
}

function parentDirectoryOf(fullPath: string): string {
  const trimmed = fullPath.trim().replace(/[/\\]+$/, "");
  if (!trimmed) return "";
  const index = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
  return index < 0 ? "" : trimmed.slice(0, index);
}

function sanitizeDirSegment(name: string) {
  return (name.trim() || "untitled")
    .replace(/[\\/:*?"<>|]/g, "-")
    .replace(/\s+/g, " ")
    .trim() || "untitled";
}

function lastPathSegment(path: string) {
  const trimmed = path.trim().replace(/[/\\]+$/, "");
  return trimmed.split(/[/\\]/u).filter(Boolean).pop() ?? "";
}

interface NewProjectModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreate: (name: string, path: string) => Promise<void>;
}

type FieldErrors = {
  path: boolean;
  name: boolean;
};

const emptyFieldErrors = (): FieldErrors => ({ path: false, name: false });

export function NewProjectModal({ open: isOpen, onOpenChange, onCreate }: NewProjectModalProps) {
  const { t } = useTranslation();
  const [parentBase, setParentBase] = useState("");
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [pathAuto, setPathAuto] = useState(true);
  const [fieldErrors, setFieldErrors] = useState<FieldErrors>(emptyFieldErrors);
  const [busy, setBusy] = useState(false);

  function clearFieldErrors() {
    setFieldErrors(emptyFieldErrors());
  }

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    clearFieldErrors();
    setName(DEFAULT_PROJECT_NAME);
    setPath("");
    setParentBase("");
    setPathAuto(true);

    (async () => {
      try {
        const parent = formatDisplayPath(await ProjectService.defaultProjectParentDirectory());
        if (cancelled) return;
        const defaultName = DEFAULT_PROJECT_NAME;
        setParentBase(parent);
        setName(defaultName);
        setPath(joinPath(parent, sanitizeDirSegment(defaultName)));
      } catch (error) {
        if (cancelled) return;
        setPathAuto(false);
        toast.error(formatErrorMessage(error));
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  function updateName(nextName: string) {
    setName(nextName);
    clearFieldErrors();
    if (!pathAuto) return;
    setPath(joinPath(parentBase, sanitizeDirSegment(nextName)));
  }

  async function browseParentDirectory() {
    clearFieldErrors();
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("projectPicker.newProjectModal.browseTitle"),
        defaultPath: parentBase || undefined,
      });
      if (!selected || Array.isArray(selected)) return;
      const parent = formatDisplayPath(selected);
      setParentBase(parent);
      setPathAuto(true);
      setPath(joinPath(parent, sanitizeDirSegment(name)));
    } catch (error) {
      toast.error(formatErrorMessage(error));
    }
  }

  async function handleCreate() {
    clearFieldErrors();
    setBusy(true);
    try {
      await onCreate(name.trim(), path.trim());
      onOpenChange(false);
    } catch (error) {
      setFieldErrors({ path: true, name: true });
      toast.error(formatErrorMessage(error));
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
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button type="button" onClick={() => void handleCreate()} disabled={busy}>
            {busy ? t("projectPicker.creating") : t("projectPicker.newProjectModal.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
