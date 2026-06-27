import { useTranslation } from 'react-i18next';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { PinEditor } from '../shared/PinEditor';
import { DetailForm, DetailNameField, DetailReadonlyField } from '../shared/DetailForm';

interface FunctionDetailPanelProps {
  fn: {
    id: string;
    name: string;
    inputs?: Array<{ id: string; name: string; type: string; containerType?: string }>;
    outputs?: Array<{ id: string; name: string; type: string; containerType?: string }>;
  };
  onUpdate: (patch: Record<string, unknown>) => void;
}

export function FunctionDetailPanel({ fn, onUpdate }: FunctionDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: fn.name })}>
      <DetailForm>
        <DetailNameField
          label={t('detail.fields.name')}
          value={fn.name}
          onCommit={(name) => onUpdate({ name })}
        />
        <DetailReadonlyField label={t('detail.fields.type')} className="italic">
          {t('detail.typeLabels.function')}
        </DetailReadonlyField>
      </DetailForm>
      <PinEditor
        title={t('detail.pinEditor.inputs')}
        emptyMessage={t('detail.pinEditor.noInputs')}
        pins={fn.inputs ?? []}
        onChange={(inputs) => onUpdate({ inputs })}
      />
      <PinEditor
        title={t('detail.pinEditor.outputs')}
        emptyMessage={t('detail.pinEditor.noOutputs')}
        pins={fn.outputs ?? []}
        onChange={(outputs) => onUpdate({ outputs })}
      />
    </DetailPanelShell>
  );
}
