import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Table, TableBody, TableCell, TableRow } from '@/components/ui/table';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';
import { PinEditor } from '../shared/PinEditor';

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
      <Table className="text-[11px] text-[#cccccc]">
        <TableBody>
          <TableRow>
            <TableCell className="w-20 bg-white/5 font-bold text-gray-400">
              {t('detail.fields.name')}
            </TableCell>
            <TableCell>
              <Input
                className="h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none"
                value={fn.name}
                onChange={(e) => onUpdate({ name: e.target.value })}
              />
            </TableCell>
          </TableRow>
          <TableRow>
            <TableCell className="bg-white/5 font-bold text-gray-400">{t('detail.fields.type')}</TableCell>
            <TableCell className="italic text-gray-400">{t('detail.typeLabels.function')}</TableCell>
          </TableRow>
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
