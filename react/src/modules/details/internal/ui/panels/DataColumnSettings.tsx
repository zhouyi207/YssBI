import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscEdit } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Select } from "@/shared/ui";
import type { ColumnInfo, SemanticType } from "@/shared/types/domain/database";
import { changeColumnPhysical } from "@/features/application/dataManagement/databaseMutation";
import {
  captureDatabaseRead,
  type DatabaseRead,
} from "@/features/application/dataManagement/databaseRead";
import { captureProjectIdentity } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { ui } from "@/features/core/ui/ui";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DetailForm } from "../shared/DetailForm";
import { DataColumnSemanticDialog } from "./DataColumnSemanticDialog";

const PHYSICAL_TYPES = [
  "Bool",
  "Int8",
  "Int16",
  "Int32",
  "Int64",
  "UInt8",
  "UInt16",
  "UInt32",
  "UInt64",
  "Float32",
  "Float64",
  "Utf8",
  "Date",
  "Datetime(s)",
  "Datetime(ms)",
  "Datetime(us)",
  "Datetime(ns)",
  "Time",
  "Dictionary(Int32, Utf8)",
];

export function DataColumnSettings({
  databaseId,
  column,
}: {
  databaseId: string;
  column: ColumnInfo;
}) {
  const { t } = useTranslation();
  const id = useId();
  const currentPhysical = column.physical;
  const semanticTypes = column.supportedSemanticTypes;
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const active = useRef(true);
  const [error, setError] = useState(false);
  const [semanticDialog, setSemanticDialog] = useState<{
    kind: SemanticType;
    read: DatabaseRead;
  } | null>(null);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);

  const confirmPhysical = async (next: string) => {
    if (next === currentPhysical || pending.current) return;
    pending.current = true;
    setBusy(true);
    setError(false);
    try {
      const read = captureDatabaseRead(captureProjectIdentity(), databaseId, () => active.current);
      if (!read.isCurrent()) return;
      const confirmed = await ui.confirm({
        title: t("detail.data.confirmPhysicalTitle"),
        message: t("detail.data.confirmPhysicalMessage", {
          column: column.name,
          from: currentPhysical,
          to: next,
        }),
        confirmText: t("common.confirm"),
        cancelText: t("common.cancel"),
      });
      if (confirmed && read.isCurrent()) await changeColumnPhysical(databaseId, column.name, next);
    } catch (error) {
      if (active.current) {
        setError(true);
        reportViewIssue("data", error, "DataColumnSettings");
      }
    } finally {
      pending.current = false;
      if (active.current) setBusy(false);
    }
  };
  const openSemantic = (kind: SemanticType) => {
    if (pending.current || semanticDialog || !semanticTypes.includes(kind)) return;
    try {
      const read = captureDatabaseRead(captureProjectIdentity(), databaseId, () => active.current);
      if (read.isCurrent()) {
        setError(false);
        setSemanticDialog({ kind, read });
      }
    } catch (error) {
      setError(true);
      reportViewIssue("data", error, "DataColumnSettings");
    }
  };
  const kind = column.semantic?.kind;
  const configurable =
    kind !== undefined &&
    semanticTypes.includes(kind) &&
    (kind === "Numeric" || kind === "Categorical" || kind === "Ordinal" || kind === "Binary");
  const editLabel = t(
    kind === "Numeric" ? "detail.data.editConstraints" : "detail.data.editMapping",
  );

  return (
    <>
      <DetailForm>
        <fieldset disabled={busy} className="min-w-0 space-y-1">
          <DetailFieldRow label={<label htmlFor={id + "-physical"}>Physical</label>}>
            <Select
              id={id + "-physical"}
              disabled={busy}
              value={currentPhysical}
              onChange={(next) => void confirmPhysical(next)}
              options={[...new Set([currentPhysical, ...PHYSICAL_TYPES])].map((value) => ({
                value,
                label: value,
              }))}
            />
          </DetailFieldRow>
          <DetailFieldRow
            label={<label htmlFor={id + "-semantic"}>Semantic</label>}
            valueClassName="flex items-center gap-1"
          >
            <Select
              id={id + "-semantic"}
              className="min-w-0 flex-1 [&_[data-slot=select-value]]:truncate"
              disabled={busy || semanticTypes.length === 0}
              value={kind ?? ""}
              options={[...semanticTypes]}
              onChange={(value) => openSemantic(value as SemanticType)}
            />
            {configurable && (
              <Button
                type="button"
                size="icon"
                variant="outline"
                aria-label={editLabel}
                title={editLabel}
                onClick={() => openSemantic(kind)}
              >
                <VscEdit aria-hidden />
              </Button>
            )}
          </DetailFieldRow>
        </fieldset>
        {error && (
          <p role="alert" className="text-xs text-destructive">
            {t("detail.data.updateFailed")}
          </p>
        )}
      </DetailForm>
      {semanticDialog && (
        <DataColumnSemanticDialog
          column={column}
          kind={semanticDialog.kind}
          read={semanticDialog.read}
          onClose={() => setSemanticDialog(null)}
        />
      )}
    </>
  );
}
