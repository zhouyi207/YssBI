import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
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
import { ProjectService } from "@/services/project/projectService";

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

export function NewProjectModal({ open: isOpen, onOpenChange, onCreate }: NewProjectModalProps) {
  const { t } = useTranslation();
  const [parentBase, setParentBase] = useState("");
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [pathAuto, setPathAuto] = useState(true);
  const [pathError, setPathError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [validating, setValidating] = useState(false);
  const validateTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const validateSeq = useRef(0);

  const validatePath = useCallback(async (nextPath: string) => {
    if (!nextPath.trim()) {
      setPathError(t("projectPicker.newProjectModal.pathRequired"));
      return;
    }
    try {
      const result = await ProjectService.validateNewProjectPath(nextPath);
      setPathError(result.ok ? null : result.message ?? t("projectPicker.newProjectModal.invalidPath"));
    } catch (error) {
      setPathError(error instanceof Error ? error.message : String(error));
    }
  }, [t]);

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    setNotice(null);
    setPathError(null);
    setName(t("projectPicker.newProjectModal.defaultName"));
    setPath("");
    setParentBase("");
    setPathAuto(true);

    (async () => {
      try {
        const parent = await ProjectService.defaultProjectParentDirectory();
        if (cancelled) return;
        const defaultName = t("projectPicker.newProjectModal.defaultName");
        setParentBase(parent);
        setName(defaultName);
        setPath(joinPath(parent, sanitizeDirSegment(defaultName)));
      } catch (error) {
        if (cancelled) return;
        setPathAuto(false);
        setNotice(error instanceof Error ? error.message : String(error));
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [isOpen, t]);

  useEffect(() => {
    if (!isOpen) return;
    const seq = ++validateSeq.current;
    setValidating(true);
    if (validateTimer.current) clearTimeout(validateTimer.current);
    validateTimer.current = setTimeout(() => {
      void validatePath(path).finally(() => {
        if (validateSeq.current === seq) {
          setValidating(false);
        }
      });
    }, 250);
    return () => {
      validateSeq.current += 1;
      if (validateTimer.current) clearTimeout(validateTimer.current);
    };
  }, [isOpen, path, validatePath]);

  function updateName(nextName: string) {
    setName(nextName);
    setNotice(null);
    if (!pathAuto) return;
    setPath(joinPath(parentBase, sanitizeDirSegment(nextName)));
  }

  async function browseParentDirectory() {
    setNotice(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("projectPicker.newProjectModal.browseTitle"),
        defaultPath: parentBase || undefined,
      });
      if (!selected || Array.isArray(selected)) return;
      setParentBase(selected);
      setPathAuto(true);
      setPath(joinPath(selected, sanitizeDirSegment(name)));
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleCreate() {
    const trimmedName = name.trim();
    const trimmedPath = path.trim();
    setNotice(null);
    if (!trimmedName) {
      setNotice(t("projectPicker.newProjectModal.nameRequired"));
      return;
    }
    if (!trimmedPath) {
      setNotice(t("projectPicker.newProjectModal.pathRequired"));
      return;
    }

    setBusy(true);
    try {
      const result = await ProjectService.validateNewProjectPath(trimmedPath);
      if (!result.ok) {
        setPathError(result.message ?? t("projectPicker.newProjectModal.invalidPath"));
        setNotice(t("projectPicker.newProjectModal.fixErrors"));
        return;
      }
      await onCreate(trimmedName, trimmedPath);
      onOpenChange(false);
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  const canCreate = useMemo(
    () => !busy && !validating && Boolean(name.trim()) && Boolean(path.trim()) && !pathError,
    [busy, name, path, pathError, validating],
  );

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
          {notice ? (
            <p className="rounded border border-destructive/40 bg-destructive/10 px-2 py-1.5 text-[12px] text-destructive">
              {notice}
            </p>
          ) : null}

          <div className="space-y-1">
            <Label htmlFor="new-project-name" className="text-[12px] text-muted-foreground">
              {t("projectPicker.newProjectModal.nameLabel")}
            </Label>
            <Input
              id="new-project-name"
              value={name}
              aria-invalid={!name.trim()}
              onChange={(event) => updateName(event.target.value)}
              className="bg-muted/50 text-[13px] text-foreground"
              autoComplete="off"
            />
          </div>

          <div className="space-y-1">
            <Label htmlFor="new-project-path" className="text-[12px] text-muted-foreground">
              {t("projectPicker.newProjectModal.pathLabel")}
            </Label>
            <div className="flex gap-2">
              <Input
                id="new-project-path"
                value={path}
                aria-invalid={Boolean(pathError)}
                onChange={(event) => {
                  const nextPath = event.target.value;
                  setNotice(null);
                  setPathAuto(false);
                  setPath(nextPath);
                  const nextName = lastPathSegment(nextPath);
                  if (nextName) setName(nextName);
                  setParentBase(parentDirectoryOf(nextPath));
                }}
                className={[
                  "min-w-0 flex-1 bg-muted/50 font-mono text-[12px] text-foreground",
                  pathError ? "border-destructive/80" : "border-input",
                ].join(" ")}
                autoComplete="off"
                spellCheck={false}
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => void browseParentDirectory()}
                className="shrink-0 border-border bg-muted text-[12px] text-foreground hover:bg-muted/80"
              >
                {t("projectPicker.newProjectModal.browse")}
              </Button>
            </div>
            <div className="min-h-[1.25rem] text-[11px] leading-snug" aria-live="polite">
              {validating ? (
                <p className="text-muted-foreground">{t("projectPicker.newProjectModal.validating")}</p>
              ) : pathError ? (
                <p className="text-destructive">{pathError}</p>
              ) : (
                <span className="text-transparent select-none" aria-hidden>
                  .
                </span>
              )}
            </div>
          </div>
        </div>

        <DialogFooter className="gap-2 sm:justify-end">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button type="button" onClick={() => void handleCreate()} disabled={!canCreate}>
            {busy ? t("projectPicker.creating") : t("projectPicker.newProjectModal.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
