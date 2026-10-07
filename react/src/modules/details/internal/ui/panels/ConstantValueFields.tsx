import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscEdit } from "react-icons/vsc";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Select } from "@/shared/ui";
import {
  dataTypeKind,
  dataTypeDisplay,
  dataTypeFromKey,
  isPrimitiveType,
  isComplexType,
  CONSTANT_SELECTABLE_DATA_TYPE_KINDS,
  DATA_SERIES_ELEMENT_TYPE_KINDS,
} from "@/shared/types/domain/valueType";
import { dataValueToRaw } from "@/shared/types/domain/dataValue";
import {
  parseConstantValueInput,
  type ConstantValueInputError,
} from "@/features/domain/graphConstants/valueInput";
import { DetailCommitInput } from "../shared/DetailForm";
import { ConstantValueEditorModal } from "../constantValue/ConstantValueEditorModal";
import { formatConstantValueSummary } from "../constantValue/constantValuePresentation";

interface ConstantValueFieldsProps {
  constant: {
    id: string;
    name: string;
    dataType: import("@/shared/types/domain/valueType").ValueType;
    dataValue: import("@/shared/types/domain/dataValue").DataValue;
  };
  onUpdate: (
    patch: Partial<Pick<ConstantValueFieldsProps["constant"], "name" | "dataType" | "dataValue">>,
  ) => void;
}

export function ConstantValueFields({ constant, onUpdate }: ConstantValueFieldsProps) {
  const { t } = useTranslation();
  const typeInputId = useId();
  const valueErrorId = useId();
  const [valueError, setValueError] = useState<ConstantValueInputError | null>(null);
  const [valueEditorOpen, setValueEditorOpen] = useState(false);
  useEffect(() => setValueError(null), [constant.dataType, constant.dataValue]);
  const commitValue = (raw: string | boolean) => {
    const result = parseConstantValueInput(raw, constant.dataType);
    if (!result.ok) {
      setValueError(result.error);
      return;
    }
    setValueError(null);
    onUpdate({ dataValue: result.value });
  };
  const numeric = dataTypeKind(constant.dataType) === "Numeric";
  const typeOptions = CONSTANT_SELECTABLE_DATA_TYPE_KINDS.flatMap((kind) =>
    kind === "DataSeries"
      ? DATA_SERIES_ELEMENT_TYPE_KINDS.map((inner) => `DataSeries<${inner}>`)
      : [kind],
  );
  if (constant.dataType.kind === "Array") {
    typeOptions.push(...DATA_SERIES_ELEMENT_TYPE_KINDS.map((inner) => `Array<${inner}>`));
  }

  const valueSummary = formatConstantValueSummary(
    constant.dataType,
    constant.dataValue,
    t("detail.constantValue.empty"),
  );

  return (
    <>
      <div className="grid min-w-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1.25fr)_minmax(0,1fr)] items-center gap-1">
        <label className="min-w-0">
          <span className="sr-only">{t("detail.fields.name")}</span>
          <DetailCommitInput
            className="h-7 min-w-0 px-2 text-xs shadow-none"
            value={constant.name}
            onCommit={(name) => onUpdate({ name })}
          />
        </label>
        <div className="min-w-0">
          <Label htmlFor={typeInputId} className="sr-only">
            {t("detail.fields.type")}
          </Label>
          <Select
            id={typeInputId}
            className="min-w-0 gap-1 px-2 text-xs shadow-none [&_[data-slot=select-value]]:truncate [&_svg]:size-3"
            value={dataTypeDisplay(constant.dataType)}
            options={typeOptions}
            onChange={(value) => onUpdate({ dataType: dataTypeFromKey(value) })}
          />
        </div>
        {isPrimitiveType(constant.dataType) && (
          <div className="min-w-0">
            {dataTypeKind(constant.dataType) === "Binary" ? (
              <div className="flex h-7 items-center gap-1.5 px-2">
                <Checkbox
                  id={`constant-bool-${constant.id}`}
                  aria-label={t("detail.fields.value")}
                  checked={!!dataValueToRaw(constant.dataValue)}
                  onCheckedChange={(checked) => commitValue(checked === true)}
                />
                <Label htmlFor={`constant-bool-${constant.id}`} className="text-xs font-normal">
                  {String(!!dataValueToRaw(constant.dataValue))}
                </Label>
              </div>
            ) : (
              <label className="block min-w-0">
                <span className="sr-only">{t("detail.fields.value")}</span>
                <DetailCommitInput
                  className="h-7 min-w-0 px-2 text-xs shadow-none"
                  type={numeric ? "number" : "text"}
                  value={
                    numeric && "value" in constant.dataValue
                      ? String(constant.dataValue.value)
                      : String(dataValueToRaw(constant.dataValue) ?? "")
                  }
                  onCommit={commitValue}
                  aria-invalid={Boolean(valueError)}
                  aria-describedby={valueError ? valueErrorId : undefined}
                />
              </label>
            )}
          </div>
        )}
        {isComplexType(constant.dataType) && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 min-w-0 justify-between px-2 font-mono"
            aria-label={`${t("detail.constantValue.edit")}: ${valueSummary}`}
            onClick={() => setValueEditorOpen(true)}
          >
            <span className="truncate">{valueSummary}</span>
            <VscEdit aria-hidden />
          </Button>
        )}
        {valueError && (
          <p id={valueErrorId} role="alert" className="col-span-3 text-xs text-destructive">
            {t(`detail.constantValue.errors.${valueError}`)}
          </p>
        )}
      </div>

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
