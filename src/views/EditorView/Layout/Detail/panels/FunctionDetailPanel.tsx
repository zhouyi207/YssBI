import { useTranslation } from 'react-i18next';
import type { FunctionPinSpec, FunctionSignaturePatch } from '@/shared/types';

import { GraphTraceDetails } from '../observability/GraphTraceDetails';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { PinEditor } from '../shared/PinEditor';

import { DetailForm, DetailNameField, DetailReadonlyField } from '../shared/DetailForm';

interface FunctionDetailPanelProps {
  fn: {
    path: string;
    name: string;
    inputs?: FunctionPinSpec[];
    outputs?: FunctionPinSpec[];
  };

  onRename: (name: string) => void;
  onSignatureChange: (patch: FunctionSignaturePatch) => void;
}

export function FunctionDetailPanel({
  fn,

  onRename,
  onSignatureChange,
}: FunctionDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: fn.name })}>
      <DetailForm>
        <DetailNameField
          label={t('detail.fields.name')}
          value={fn.name}
          onCommit={onRename}
        />
        <DetailReadonlyField label={t('detail.fields.type')} className="italic">
          {t('detail.typeLabels.function')}
        </DetailReadonlyField>
      </DetailForm>
      <PinEditor
        title={t('detail.pinEditor.inputs')}
        emptyMessage={t('detail.pinEditor.noInputs')}
        pins={fn.inputs ?? []}
        onChange={(inputs) => onSignatureChange({ inputs })}
      />
      <PinEditor
        title={t('detail.pinEditor.outputs')}
        emptyMessage={t('detail.pinEditor.noOutputs')}
        pins={fn.outputs ?? []}
        onChange={(outputs) => onSignatureChange({ outputs })}
      />
      <GraphTraceDetails graphPath={fn.path} />
    </DetailPanelShell>
  );
}
