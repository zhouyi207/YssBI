import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Select } from "@/shared/ui";
import {
  dataTypeKind,
  dataTypeFromKey,
  isPrimitiveType,
  isComplexType,
  CONSTANT_SELECTABLE_DATA_TYPE_KINDS,
  DATA_SERIES_ELEMENT_TYPE_KINDS,
} from "@/shared/types/domain/dataType";
import { dataValueToRaw, dataValueFromRaw } from "@/shared/types/domain/dataValue";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DetailCommitInput, DetailForm } from "../shared/DetailForm";
import { DetailText } from "../shared/DetailText";
import { detailInlineInputClass } from "../shared/detailStyles";
import { ConstantValueEditorModal } from "../constantValue/ConstantValueEditorModal";
import { formatConstantValueSummary } from "../constantValue/constantValueUtils";

interface ConstantValueFieldsProps {
  constant: {
    id: string;
    name: string;
    dataType: import("@/shared/types/domain/dataType").DataType;
    dataValue: import("@/shared/types/domain/dataValue").DataValue;
  };
  onUpdate: (
    patch: Partial<Pick<ConstantValueFieldsProps["constant"], "name" | "dataType" | "dataValue">>,
  ) => void;
}

export function ConstantValueFields({ constant, onUpdate }: ConstantValueFieldsProps) {
  const { t } = useTranslation();
  const [valueEditorOpen, setValueEditorOpen] = useState(false);
  const numeric = constant.dataType.kind === "Int64" || constant.dataType.kind === "Float64";

  const valueSummary = formatConstantValueSummary(
    constant.dataType,
    constant.dataValue,
    t("detail.constantValue.empty"),
  );

  return (
    <>
      <DetailForm>
        <DetailFieldRow label={t("detail.fields.name")}>
          <DetailCommitInput value={constant.name} onCommit={(name) => onUpdate({ name })} />
        </DetailFieldRow>
        <DetailFieldRow label={t("detail.fields.type")}>
          <Select
            value={dataTypeKind(constant.dataType)}
            options={CONSTANT_SELECTABLE_DATA_TYPE_KINDS.map((kind) => ({
              label: kind,
              value: kind,
            }))}
            onChange={(val) =>
              onUpdate({
                dataType: dataTypeFromKey(val, {
                  kind: val === "DataSeries" ? "Float64" : "Int64",
                }),
              })
            }
          />
        </DetailFieldRow>
        {(constant.dataType.kind === "Array" || constant.dataType.kind === "DataSeries") && (
          <DetailFieldRow label={t("detail.fields.elementType")}>
            <Select
              value={constant.dataType.inner.kind}
              options={DATA_SERIES_ELEMENT_TYPE_KINDS.map((kind) => ({ label: kind, value: kind }))}
              onChange={(value) =>
                onUpdate({
                  dataType: {
                    kind: constant.dataType.kind as "Array" | "DataSeries",
                    inner: dataTypeFromKey(value),
                  },
                })
              }
            />
          </DetailFieldRow>
        )}
        {isPrimitiveType(constant.dataType) && (
          <DetailFieldRow label={t("detail.fields.value")}>
            {constant.dataType.kind === "Boolean" ? (
              <div className="flex items-center justify-end gap-2">
                <Checkbox
                  id={`constant-bool-${constant.id}`}
                  checked={!!dataValueToRaw(constant.dataValue)}
                  onCheckedChange={(checked) =>
                    onUpdate({ dataValue: dataValueFromRaw(checked === true, constant.dataType) })
                  }
                />
                <Label htmlFor={`constant-bool-${constant.id}`} className="text-sm font-normal">
                  {String(!!dataValueToRaw(constant.dataValue))}
                </Label>
              </div>
            ) : (
              <DetailCommitInput
                className={detailInlineInputClass}
                type={numeric ? "number" : "text"}
                value={String(dataValueToRaw(constant.dataValue) ?? "")}
                onCommit={(draft) => {
                  const val = numeric ? Number(draft) : draft;
                  onUpdate({ dataValue: dataValueFromRaw(val, constant.dataType) });
                }}
              />
            )}
          </DetailFieldRow>
        )}
        {isComplexType(constant.dataType) && (
          <DetailFieldRow label={t("detail.fields.value")}>
            <div className="flex min-w-0 items-center gap-2">
              <DetailText
                tone="muted"
                className="min-h-8 min-w-0 flex-1 truncate rounded-md border border-transparent px-3 py-1 font-mono text-xs"
              >
                {valueSummary}
              </DetailText>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => setValueEditorOpen(true)}
              >
                {t("detail.constantValue.edit")}
              </Button>
            </div>
          </DetailFieldRow>
        )}
      </DetailForm>

      <ConstantValueEditorModal
        open={valueEditorOpen}
        onClose={() => setValueEditorOpen(false)}
        dataType={constant.dataType}
        dataValue={constant.dataValue}
        onSave={(dataValue) => onUpdate({ dataValue })}
      />
    </>
  );
}
