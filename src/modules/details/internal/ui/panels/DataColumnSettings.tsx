import { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/shared/ui";
import {
  SEMANTIC_TYPES,
  type ColumnInfo,
  type ColumnSemantic,
  type SemanticType,
} from "@/shared/types/domain/database";
import {
  changeColumnPhysical,
  changeColumnSemantic,
} from "@/features/application/dataManagement/databaseMutation";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DetailForm } from "../shared/DetailForm";

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
  const currentPhysical = column.physical ?? column.type;
  const [physical, setPhysical] = useState(currentPhysical);
  const [semantic, setSemantic] = useState<ColumnSemantic | null>(column.semantic ?? null);
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const [error, setError] = useState(false);
  const apply = async (action: () => Promise<unknown>) => {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setError(false);
    try {
      await action();
    } catch (error) {
      setError(true);
      reportViewIssue("data", error, "DataColumnSettings");
    } finally {
      pending.current = false;
      setBusy(false);
    }
  };
  const selectSemantic = (kind: SemanticType) => {
    if (kind === semantic?.kind) return;
    const domain = kind === "Categorical" || kind === "Ordinal" || kind === "Binary";
    setSemantic({
      kind,
      values: domain ? (semantic?.values ?? []) : [],
      positiveValue: null,
      numeric: kind === "Numeric" ? { integer: false, minimum: null, maximum: null } : null,
    });
  };
  const values = semantic?.values ?? [];
  const domain = semantic && ["Categorical", "Ordinal", "Binary"].includes(semantic.kind);
  const updateValue = (index: number, key: "value" | "label", value: string) => {
    if (!semantic) return;
    setSemantic({
      ...semantic,
      values: values.map((entry, i) => (i === index ? { ...entry, [key]: value } : entry)),
      positiveValue:
        key === "value" && semantic.positiveValue === values[index]?.value
          ? value
          : semantic.positiveValue,
    });
  };
  const move = (index: number, offset: number) => {
    if (!semantic) return;
    const next = [...values];
    [next[index], next[index + offset]] = [next[index + offset]!, next[index]!];
    setSemantic({ ...semantic, values: next });
  };

  return (
    <DetailForm>
      <fieldset disabled={busy} className="min-w-0 space-y-2">
        <DetailFieldRow label={<label htmlFor={`${id}-physical`}>Physical</label>}>
          <Select
            id={`${id}-physical`}
            disabled={busy}
            value={physical}
            onChange={setPhysical}
            options={[...new Set([currentPhysical, ...PHYSICAL_TYPES])].map((value) => ({
              value,
              label: value === "Boolean" ? "Bool" : value === "String" ? "Utf8" : value,
            }))}
          />
        </DetailFieldRow>
        <Button
          size="sm"
          variant="outline"
          disabled={busy || physical === currentPhysical}
          onClick={() => void apply(() => changeColumnPhysical(databaseId, column.name, physical))}
        >
          {t("detail.data.applyPhysical")}
        </Button>
        <DetailFieldRow label={<label htmlFor={`${id}-semantic`}>Semantic</label>}>
          <Select
            id={`${id}-semantic`}
            disabled={busy}
            value={semantic?.kind ?? ""}
            options={[...SEMANTIC_TYPES]}
            onChange={(value) => selectSemantic(value as SemanticType)}
          />
        </DetailFieldRow>
        {domain && (
          <div className="space-y-2">
            <p className="text-xs text-muted-foreground">
              {t(
                semantic.kind === "Ordinal"
                  ? "detail.data.ordinalHint"
                  : semantic.kind === "Binary"
                    ? "detail.data.binaryHint"
                    : "detail.data.categoryHint",
              )}
            </p>
            {values.map((entry, index) => (
              <div key={index} className="flex min-w-0 items-center gap-1">
                {semantic.kind === "Ordinal" && <span className="text-xs">{index + 1}</span>}
                <Input
                  aria-label={t("detail.data.value", { index: index + 1 })}
                  value={entry.value}
                  placeholder={t("detail.fields.value")}
                  onChange={(event) => updateValue(index, "value", event.target.value)}
                />
                <Input
                  aria-label={t("detail.data.label", { index: index + 1 })}
                  value={entry.label}
                  placeholder={t("detail.data.labelPlaceholder")}
                  onChange={(event) => updateValue(index, "label", event.target.value)}
                />
                {semantic.kind === "Ordinal" && (
                  <>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={busy || index === 0}
                      aria-label={t("detail.parameterEditor.moveColumnUp", { column: entry.label })}
                      onClick={() => move(index, -1)}
                    >
                      ↑
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={busy || index === values.length - 1}
                      aria-label={t("detail.parameterEditor.moveColumnDown", {
                        column: entry.label,
                      })}
                      onClick={() => move(index, 1)}
                    >
                      ↓
                    </Button>
                  </>
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  aria-label={t("detail.data.removeValue", { index: index + 1 })}
                  onClick={() =>
                    setSemantic({
                      ...semantic,
                      values: values.filter((_, i) => i !== index),
                      positiveValue:
                        semantic.positiveValue === entry.value ? null : semantic.positiveValue,
                    })
                  }
                >
                  ×
                </Button>
              </div>
            ))}
            <Button
              size="sm"
              variant="outline"
              disabled={busy || (semantic.kind === "Binary" && values.length >= 2)}
              onClick={() =>
                setSemantic({ ...semantic, values: [...values, { value: "", label: "" }] })
              }
            >
              {t("detail.data.addValue")}
            </Button>
          </div>
        )}
        {semantic?.kind === "Binary" && (
          <DetailFieldRow label={t("detail.data.positive")}>
            <Select
              disabled={busy}
              value={
                semantic.positiveValue === null
                  ? "none"
                  : String(values.findIndex((entry) => entry.value === semantic.positiveValue))
              }
              options={[
                { value: "none", label: t("detail.data.noPositive") },
                ...values.map((entry, index) => ({
                  value: String(index),
                  label: entry.label || entry.value || String(index + 1),
                })),
              ]}
              onChange={(value) =>
                setSemantic({
                  ...semantic,
                  positiveValue: value === "none" ? null : values[Number(value)]!.value,
                })
              }
            />
          </DetailFieldRow>
        )}
        {semantic?.kind === "Numeric" && (
          <div className="space-y-2">
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                checked={semantic.numeric?.integer ?? false}
                onChange={(event) =>
                  setSemantic({
                    ...semantic,
                    numeric: {
                      minimum: null,
                      maximum: null,
                      ...semantic.numeric,
                      integer: event.target.checked,
                    },
                  })
                }
              />
              {t("detail.data.integer")}
            </label>
            {(["minimum", "maximum"] as const).map((key) => (
              <DetailFieldRow
                key={key}
                label={<label htmlFor={`${id}-${key}`}>{t(`detail.data.${key}`)}</label>}
              >
                <Input
                  id={`${id}-${key}`}
                  value={semantic.numeric?.[key] ?? ""}
                  onChange={(event) =>
                    setSemantic({
                      ...semantic,
                      numeric: {
                        integer: false,
                        minimum: null,
                        maximum: null,
                        ...semantic.numeric,
                        [key]: event.target.value || null,
                      },
                    })
                  }
                />
              </DetailFieldRow>
            ))}
          </div>
        )}
        <Button
          size="sm"
          variant="outline"
          disabled={
            busy ||
            !semantic ||
            JSON.stringify(semantic) === JSON.stringify(column.semantic) ||
            (semantic.kind === "Binary" && values.length !== 2)
          }
          onClick={() =>
            semantic && void apply(() => changeColumnSemantic(databaseId, column.name, semantic))
          }
        >
          {t("detail.data.applySemantic")}
        </Button>
      </fieldset>
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {t("detail.data.updateFailed")}
        </p>
      )}
    </DetailForm>
  );
}
