import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Table, TableBody } from '@/components/ui/table';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { PinEditor } from '../shared/PinEditor';
import { detailInlineInputClass, detailTableClass, detailValueMutedClass } from '../shared/detailStyles';

interface FunctionDetailPanelProps {
  fn: {
    id: string;
    name: string;
    inputs?: Array<{ id: string; name: string; type: string; containerType?: string }>;
    outputs?: Array<{ id: string; name: string; type: string; containerType?: string }>;
  };
  onUpdate: (patch: Record<string, unknown>) => void;
  onDelete: () => Promise<void>;
  onDeleted: () => void;
}

export function FunctionDetailPanel({ fn, onUpdate, onDelete, onDeleted }: FunctionDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: fn.name })}>
      <Table className={detailTableClass}>
        <TableBody>
          <DetailFieldRow label={t('detail.fields.name')}>
            <Input
              className={detailInlineInputClass}
              value={fn.name}
              onChange={(e) => onUpdate({ name: e.target.value })}
            />
          </DetailFieldRow>
          <DetailFieldRow label={t('detail.fields.type')} valueClassName={`italic ${detailValueMutedClass}`}>
            {t('detail.typeLabels.function')}
          </DetailFieldRow>
        </TableBody>
      </Table>
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
      <DetailDeleteButton
        itemType="function"
        itemName={fn.name}
        onDelete={onDelete}
        onDeleted={onDeleted}
      />
    </DetailPanelShell>
  );
}
