import { useTranslation } from 'react-i18next';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { Select } from '@/shared/ui';
import { dataTypeKind, dataTypeFromKey, isPrimitiveType } from '@/shared/types/domain/dataType';
import { dataValueToRaw, dataValueFromRaw } from '@/shared/types/domain/dataValue';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { DetailCommitInput, DetailForm, DetailNameField } from '../shared/DetailForm';
import { detailInlineInputClass } from '../shared/detailStyles';

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

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: variable.name })}>
      <DetailForm>
        <DetailNameField
          label={t('detail.fields.name')}
          value={variable.name}
          onCommit={(name) => onUpdate({ name })}
        />
        <DetailFieldRow label={t('detail.fields.type')}>
          <Select
            value={dataTypeKind(variable.dataType)}
            options={[
              { label: 'Boolean', value: 'Boolean' },
              { label: 'Int32', value: 'Int32' },
              { label: 'Int64', value: 'Int64' },
              { label: 'Float32', value: 'Float32' },
              { label: 'Float64', value: 'Float64' },
              { label: 'String', value: 'String' },
              { label: 'Object', value: 'Object' },
              { label: 'Any', value: 'Any' },
              { label: 'DataFrame', value: 'DataFrame' },
              { label: 'Array', value: 'Array' },
            ]}
            onChange={(val) => onUpdate({ dataType: dataTypeFromKey(val as string) })}
          />
        </DetailFieldRow>
        {variable.dataType.kind !== 'Array' && isPrimitiveType(variable.dataType) && (
          <DetailFieldRow label={t('detail.fields.value')}>
            {variable.dataType.kind === 'Boolean' ? (
              <div className="flex items-center gap-2">
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
      </DetailForm>
    </DetailPanelShell>
  );
}
