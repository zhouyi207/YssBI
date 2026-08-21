import { useTranslation } from 'react-i18next';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';

interface EventDetailPanelProps {
  event: { name: string };
}

export function EventDetailPanel({ event }: EventDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.name')} tone="body">
          {event.name}
        </DetailReadonlyField>
      </DetailForm>
    </DetailPanelShell>
  );
}
