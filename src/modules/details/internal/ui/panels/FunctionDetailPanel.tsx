import { useTranslation } from "react-i18next";
import type { FunctionPinSpec, FunctionSignaturePatch } from "@/shared/types";

import { DetailPanelShell } from "../shared/DetailPanelShell";
import { PinEditor } from "../shared/PinEditor";

import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";
import { GraphConstantsPanel } from "./GraphConstantsPanel";

interface FunctionDetailPanelProps {
  graphPath: string;
  fn: {
    name: string;
    inputs?: FunctionPinSpec[];
    outputs?: FunctionPinSpec[];
  };

  onSignatureChange: (patch: FunctionSignaturePatch) => void;
}

export function FunctionDetailPanel({
  fn,
  graphPath,
  onSignatureChange,
}: FunctionDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t("detail.fields.name")} tone="body">
          {fn.name}
        </DetailReadonlyField>
        <DetailReadonlyField label={t("detail.fields.type")} className="italic">
          {t("detail.typeLabels.function")}
        </DetailReadonlyField>
      </DetailForm>
      <GraphConstantsPanel key={graphPath} graphPath={graphPath} />
      <PinEditor
        title={t("detail.pinEditor.inputs")}
        emptyMessage={t("detail.pinEditor.noInputs")}
        pins={fn.inputs ?? []}
        onChange={(inputs) => onSignatureChange({ inputs })}
      />
      <PinEditor
        title={t("detail.pinEditor.outputs")}
        emptyMessage={t("detail.pinEditor.noOutputs")}
        pins={fn.outputs ?? []}
        onChange={(outputs) => onSignatureChange({ outputs })}
      />
    </DetailPanelShell>
  );
}
