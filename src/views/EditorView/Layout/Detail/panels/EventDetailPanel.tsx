import { useTranslation } from 'react-i18next';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm, DetailNameField } from '../shared/DetailForm';

interface EventDetailPanelProps {
  event: { path: string; name: string };
  onUpdate: (patch: Record<string, unknown>) => void;
}

export function EventDetailPanel({ event, onUpdate }: EventDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: event.name })}>
      <DetailForm>
        <DetailNameField
          label={t('detail.fields.name')}
          value={event.name}
          onCommit={(name) => onUpdate({ name })}
        />
      </DetailForm>
    </DetailPanelShell>
  );
}
