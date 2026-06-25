import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { Table, TableBody } from '@/components/ui/table';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailDeleteButton } from '../shared/DetailDeleteButton';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { detailInlineInputClass, detailTableClass } from '../shared/detailStyles';

interface EventDetailPanelProps {
  event: { id: string; name: string };
  onUpdate: (patch: Record<string, unknown>) => void;
  onDelete: () => Promise<void>;
  onDeleted: () => void;
}

export function EventDetailPanel({ event, onUpdate, onDelete, onDeleted }: EventDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: event.name })}>
      <Table className={detailTableClass}>
        <TableBody>
          <DetailFieldRow label={t('detail.fields.name')}>
            <Input
              className={detailInlineInputClass}
              value={event.name}
              onChange={(e) => onUpdate({ name: e.target.value })}
            />
          </DetailFieldRow>
        </TableBody>
      </Table>
      <DetailDeleteButton
        itemType="event"
        itemName={event.name}
        onDelete={onDelete}
        onDeleted={onDeleted}
      />
    </DetailPanelShell>
  );
}
