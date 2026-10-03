import { useEffect, useRef, useState } from "react";
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
import {
  readColumnSemanticDraft,
  type DatabaseRead,
} from "@/features/application/dataManagement/databaseRead";
import { changeColumnSemantic } from "@/features/application/dataManagement/databaseMutation";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";
import type { ColumnInfo, ColumnSemantic, SemanticType } from "@/shared/types/domain/database";
import { DataColumnSemanticFields } from "./DataColumnSemanticFields";

export function DataColumnSemanticDialog({
  column,
  kind,
  read,
  onClose,
}: {
  column: ColumnInfo;
  kind: SemanticType;
  read: DatabaseRead;
  onClose(): void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<ColumnSemantic | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<"load" | "save" | "stale" | null>(null);
  const [attempt, setAttempt] = useState(0);
  const pending = useRef(false);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    void readColumnSemanticDraft(read, column, kind)
      .then((next) => {
        if (cancelled) return;
        if (next) setDraft(next);
        else setError("stale");
      })
      .catch((error) => {
        if (cancelled) return;
        setError(read.isCurrent() ? "load" : "stale");
        reportViewIssue("data", error, "DataColumnSemanticDialog");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [read, column, kind, attempt]);

  const duplicate =
    draft && new Set(draft.values.map((entry) => entry.value)).size !== draft.values.length;
  const invalidBinary = draft?.kind === "Binary" && draft.values.length !== 2;
  const tooMany = draft && draft.values.length > 65_536;
  const confirm = async () => {
    if (!draft || loading || pending.current || duplicate || invalidBinary || tooMany) return;
    if (!read.isCurrent()) {
      setError("stale");
      return;
    }
    if (JSON.stringify(draft) === JSON.stringify(column.semantic)) {
      onClose();
      return;
    }
    pending.current = true;
    setSaving(true);
    setError(null);
    try {
      await changeColumnSemantic(read.id, column.name, draft);
      if (active.current) onClose();
    } catch (error) {
      if (active.current) {
        setError(read.isCurrent() ? "save" : "stale");
        reportViewIssue("data", error, "DataColumnSemanticDialog");
      }
    } finally {
      pending.current = false;
      if (active.current) setSaving(false);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !pending.current) onClose();
      }}
    >
      <DialogContent className="flex max-h-[85vh] w-[calc(100vw-2rem)] max-w-[640px] flex-col">
        <DialogHeader>
          <DialogTitle>{t("detail.data.confirmSemanticTitle")}</DialogTitle>
          <DialogDescription>
            {t("detail.data.confirmSemanticMessage", { column: column.name, kind })}
          </DialogDescription>
        </DialogHeader>
        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-6 py-4">
          {loading ? (
            <p role="status" className="text-sm text-muted-foreground">
              {t("common.loading")}
            </p>
          ) : (
            draft && (
              <DataColumnSemanticFields value={draft} disabled={saving} onChange={setDraft} />
            )
          )}
          {duplicate && (
            <p role="alert" className="text-sm text-destructive">
              {t("conversion.duplicateValues")}
            </p>
          )}
          {invalidBinary && (
            <p role="alert" className="text-sm text-destructive">
              {t("detail.data.binaryCount", { count: draft.values.length })}
            </p>
          )}
          {tooMany && (
            <p role="alert" className="text-sm text-destructive">
              {t("detail.data.tooManyValues")}
            </p>
          )}
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {t(
                error === "load"
                  ? "detail.data.mappingLoadFailed"
                  : error === "stale"
                    ? "detail.data.settingsStale"
                    : "detail.data.updateFailed",
              )}
            </p>
          )}
          {error === "load" && (
            <Button variant="outline" onClick={() => setAttempt((value) => value + 1)}>
              {t("common.retry")}
            </Button>
          )}
        </div>
        <DialogFooter>
          <Button variant="ghost" disabled={saving} onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            disabled={
              loading ||
              saving ||
              !draft ||
              !!duplicate ||
              invalidBinary ||
              !!tooMany ||
              error === "stale" ||
              error === "load"
            }
            onClick={() => void confirm()}
          >
            {t("common.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
