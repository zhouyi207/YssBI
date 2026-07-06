import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { getNodeDefinitionMeta } from '@/shared/types/domain/node';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import { resolveNodeDocumentationContent } from '../nodeDocumentation';
import { listDefinitionOnlyPins } from '../resolveNodePinSpecs';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';

interface NodeDefinitionDetailPanelProps {
  nodeType: string;
}

export function NodeDefinitionDetailPanel({ nodeType }: NodeDefinitionDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const definition = useNodeRegistryStore((s) => s.definitions.get(nodeType));

  const pinSpecs = useMemo(() => {
    const pins = listDefinitionOnlyPins(definition);
    return {
      inputs: pins.filter((p) => p.direction === 'input'),
      outputs: pins.filter((p) => p.direction === 'output'),
    };
  }, [definition]);

  const documentation = useMemo(() => {
    const meta = getNodeDefinitionMeta(definition);
    return resolveNodeDocumentationContent(meta, i18n.language);
  }, [definition, i18n.language]);

  if (!definition) {
    return (
      <DetailPanelShell title={t('detail.titleNodeDefinition')}>
        <DetailForm>
          <DetailReadonlyField label={t('detail.fields.type')} tone="muted">
            {nodeType}
          </DetailReadonlyField>
        </DetailForm>
      </DetailPanelShell>
    );
  }

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: definition.name })}>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.name')} tone="body" className="font-medium">
          {definition.name}
        </DetailReadonlyField>
        <DetailReadonlyField label={t('detail.fields.type')}>{definition.nodeType}</DetailReadonlyField>
        {definition.category?.length > 0 && (
          <DetailReadonlyField label={t('detail.fields.category')}>
            {definition.category.join(' / ')}
          </DetailReadonlyField>
        )}
      </DetailForm>
      <NodePinInterfacePanel
        graphId=""
        inputs={pinSpecs.inputs}
        outputs={pinSpecs.outputs}
      />
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
}
