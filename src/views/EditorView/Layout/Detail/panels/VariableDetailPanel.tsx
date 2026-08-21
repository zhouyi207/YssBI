import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { Select } from '@/shared/ui';
import { dataTypeKind, dataTypeFromKey, isPrimitiveType, isComplexType, VARIABLE_SELECTABLE_DATA_TYPE_KINDS } from '@/shared/types/domain/dataType';
import { dataValueToRaw, dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { DetailCommitInput, DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailText } from '../shared/DetailText';
import { detailInlineInputClass } from '../shared/detailStyles';
import { VariableValueEditorModal } from '../variableValue/VariableValueEditorModal';
import { formatVariableValueSummary } from '../variableValue/variableValueUtils';

interface VariableDetailPanelProps {
  variable: {
    id: string;
    name: string;
    dataType: import('@/shared/types/domain/dataType').DataType;
    dataValue: import('@/shared/types/domain/dataValue').DataValue;
  };
  onUpdate: (patch: Record<string, unknown>) => void;
}

export function VariableDetailPanel({
  variable,
  onUpdate,
}: VariableDetailPanelProps) {
  const { t } = useTranslation();
  const [valueEditorOpen, setValueEditorOpen] = useState(false);

  const valueSummary = formatVariableValueSummary(
    variable.dataType,
    variable.dataValue,
    t('detail.variableValue.empty'),
  );

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.name')} tone="body">
          {variable.name}
        </DetailReadonlyField>
        <DetailFieldRow label={t('detail.fields.type')}>
          <Select
            value={dataTypeKind(variable.dataType)}
            options={VARIABLE_SELECTABLE_DATA_TYPE_KINDS.map((kind) => ({ label: kind, value: kind }))}
            onChange={(val) => onUpdate({ dataType: dataTypeFromKey(val) })}
          />
        </DetailFieldRow>
        {variable.dataType.kind !== 'Array' && isPrimitiveType(variable.dataType) && (
          <DetailFieldRow label={t('detail.fields.value')}>
            {variable.dataType.kind === 'Boolean' ? (
              <div className="flex items-center justify-end gap-2">
                <Checkbox
                  id={`variable-bool-${variable.id}`}
                  checked={!!dataValueToRaw(variable.dataValue)}
                  onCheckedChange={(checked) =>
                    onUpdate({ dataValue: dataValueFromRaw(checked === true, variable.dataType) })
                  }
                />
                <Label htmlFor={`variable-bool-${variable.id}`} className="text-sm font-normal">
                  {String(!!dataValueToRaw(variable.dataValue))}
                </Label>
              </div>
            ) : (
              <DetailCommitInput
                className={detailInlineInputClass}
                type={variable.dataType.kind === 'String' ? 'text' : 'number'}
                value={String(dataValueToRaw(variable.dataValue) ?? '')}
                onCommit={(draft) => {
                  const val =
                    variable.dataType.kind === 'String'
                      ? draft
                      : Number(draft);
                  onUpdate({ dataValue: dataValueFromRaw(val, variable.dataType) });
                }}
              />
            )}
          </DetailFieldRow>
        )}
        {isComplexType(variable.dataType) && (
          <DetailFieldRow label={t('detail.fields.value')}>
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
                {t('detail.variableValue.edit')}
              </Button>
            </div>
          </DetailFieldRow>
        )}
      </DetailForm>

      <VariableValueEditorModal
        open={valueEditorOpen}
        onClose={() => setValueEditorOpen(false)}
        dataType={variable.dataType}
        dataValue={variable.dataValue}
        onSave={(dataValue) => onUpdate({ dataValue })}
      />
    </DetailPanelShell>
  );
}
