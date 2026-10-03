import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/shared/ui";
import type { ColumnSemantic } from "@/shared/types/domain/database";
import { DetailFieldRow } from "../shared/DetailFieldRow";

export function DataColumnSemanticFields({
  value: semantic,
  disabled,
  onChange,
}: {
  value: ColumnSemantic;
  disabled: boolean;
  onChange(value: ColumnSemantic): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [page, setPage] = useState(0);
  const { values } = semantic;
  const domain =
    semantic.kind === "Categorical" || semantic.kind === "Ordinal" || semantic.kind === "Binary";
  const lastPage = Math.max(0, Math.ceil(values.length / 50) - 1);
  const shownPage = Math.min(page, lastPage);
  const start = shownPage * 50;
  const updateValue = (index: number, key: "value" | "label", value: string) => {
    onChange({
      ...semantic,
      values: values.map((entry, row) => (row === index ? { ...entry, [key]: value } : entry)),
      positiveValue:
        key === "value" && semantic.positiveValue === values[index].value
          ? value
          : semantic.positiveValue,
    });
  };
  const move = (index: number, offset: number) => {
    const next = [...values];
    [next[index], next[index + offset]] = [next[index + offset], next[index]];
    setPage(Math.floor((index + offset) / 50));
    onChange({ ...semantic, values: next });
  };

  return (
    <fieldset disabled={disabled} className="min-w-0 space-y-3">
      {domain && (
        <>
          <p className="text-xs text-muted-foreground">{t("detail.data.initialMappingHint")}</p>
          <p className="text-xs text-muted-foreground">
            {t(
              semantic.kind === "Ordinal"
                ? "detail.data.ordinalHint"
                : semantic.kind === "Binary"
                  ? "detail.data.binaryHint"
                  : "detail.data.categoryHint",
            )}
          </p>
          <div className="space-y-2">
            <div className="flex gap-2 text-xs text-muted-foreground">
              <span className="flex-1">{t("detail.fields.value")}</span>
              <span className="flex-1">{t("detail.data.labelPlaceholder")}</span>
            </div>
            {values.slice(start, start + 50).map((entry, row) => {
              const index = start + row;
              return (
                <div key={index} className="flex min-w-0 items-center gap-1">
                  {semantic.kind === "Ordinal" && (
                    <span className="w-6 shrink-0 text-xs">{index + 1}</span>
                  )}
                  <Input
                    className="min-w-0 flex-1"
                    aria-label={t("detail.data.value", { index: index + 1 })}
                    value={entry.value}
                    onChange={(event) => updateValue(index, "value", event.target.value)}
                  />
                  <Input
                    className="min-w-0 flex-1"
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
                        disabled={disabled || index === 0}
                        aria-label={t("detail.parameterEditor.moveColumnUp", {
                          column: entry.label,
                        })}
                        onClick={() => move(index, -1)}
                      >
                        ↑
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={disabled || index === values.length - 1}
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
                      onChange({
                        ...semantic,
                        values: values.filter((_, row) => row !== index),
                        positiveValue:
                          semantic.positiveValue === entry.value ? null : semantic.positiveValue,
                      })
                    }
                  >
                    ×
                  </Button>
                </div>
              );
            })}
          </div>
          {lastPage > 0 && (
            <div className="flex items-center gap-2 text-xs">
              <Button
                variant="ghost"
                size="sm"
                disabled={disabled || shownPage === 0}
                onClick={() => setPage(shownPage - 1)}
              >
                {t("conversion.previousValues")}
              </Button>
              <span>
                {shownPage + 1}/{lastPage + 1}
              </span>
              <Button
                variant="ghost"
                size="sm"
                disabled={disabled || shownPage === lastPage}
                onClick={() => setPage(shownPage + 1)}
              >
                {t("conversion.nextValues")}
              </Button>
            </div>
          )}
          <Button
            size="sm"
            variant="outline"
            disabled={
              disabled ||
              values.length >= 65_536 ||
              (semantic.kind === "Binary" && values.length >= 2)
            }
            onClick={() => {
              setPage(Math.floor(values.length / 50));
              onChange({ ...semantic, values: [...values, { value: "", label: "" }] });
            }}
          >
            {t("detail.data.addValue")}
          </Button>
        </>
      )}
      {semantic.kind === "Binary" && values.length === 2 && (
        <DetailFieldRow
          label={<label htmlFor={id + "-positive"}>{t("detail.data.positive")}</label>}
        >
          <Select
            id={id + "-positive"}
            disabled={disabled}
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
              onChange({
                ...semantic,
                positiveValue: value === "none" ? null : values[Number(value)].value,
              })
            }
          />
        </DetailFieldRow>
      )}
      {semantic.kind === "Numeric" && (
        <>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={semantic.numeric?.integer ?? false}
              onChange={(event) =>
                onChange({
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
              label={<label htmlFor={id + "-" + key}>{t(`detail.data.${key}`)}</label>}
            >
              <Input
                id={id + "-" + key}
                value={semantic.numeric?.[key] ?? ""}
                onChange={(event) =>
                  onChange({
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
        </>
      )}
    </fieldset>
  );
}
