import { useTranslation } from 'react-i18next';
import { NODE_CATALOG_UNAVAILABLE_MESSAGE } from '@/features/application/editor/editorMutationAvailability';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';

interface NodeDefinitionDetailPanelProps {
  nodeType: string;
}

export function NodeDefinitionDetailPanel({ nodeType }: NodeDefinitionDetailPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailPanelShell title={t('detail.titleNodeDefinition')}>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.type')} tone="muted">
          {nodeType}
        </DetailReadonlyField>
        <DetailReadonlyField label={t('detail.fields.description')} tone="muted">
          {NODE_CATALOG_UNAVAILABLE_MESSAGE}
        </DetailReadonlyField>
      </DetailForm>
    </DetailPanelShell>
  );
}
