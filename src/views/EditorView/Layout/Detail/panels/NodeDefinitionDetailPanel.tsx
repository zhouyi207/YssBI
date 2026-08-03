import { useTranslation } from 'react-i18next';
import { useLocalizedNodeCatalog } from '@/features/application/nodeCatalog/useLocalizedNodeCatalog';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';

interface NodeDefinitionDetailPanelProps {
  nodeType: string;
}

export function NodeDefinitionDetailPanel({ nodeType }: NodeDefinitionDetailPanelProps) {
  const { t } = useTranslation();
  const { catalog } = useLocalizedNodeCatalog();
  const item = catalog?.items.find((candidate) => candidate.nodeTypeId === nodeType);

  return (
    <DetailPanelShell title={item?.title ?? t('detail.titleNodeDefinition')}>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.type')} tone="muted">
          {nodeType}
        </DetailReadonlyField>
        {item?.description ? (
          <DetailReadonlyField label={t('detail.fields.description')} tone="muted">
            {item.description}
          </DetailReadonlyField>
        ) : null}
      </DetailForm>
    </DetailPanelShell>
  );
}
