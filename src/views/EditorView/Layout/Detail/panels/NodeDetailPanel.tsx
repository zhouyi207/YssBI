import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import type { PinData, PinView } from '@/shared/types/store/graph';
import { getNodeDefinitionMeta } from '@/shared/types/domain/node';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { derivePinLinks } from '@/features/core/dataStore/pinLinks';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import { resolveNodeDocumentationContent } from '../nodeDocumentation';
import { resolveNodePinSpecs } from '../resolveNodePinSpecs';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailText } from '../shared/DetailText';

const EMPTY_PINS: PinData[] = [];
const EMPTY_CONNECTIONS: string[] = [];
const EMPTY_PIN_CONNECTIONS: string[][] = [];

interface NodeDetailPanelProps {
  nodeId: string;
}

export function NodeDetailPanel({ nodeId }: NodeDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const node = useGraphDataStore((s) => s.nodes[nodeId]);
  const pinObjs = useGraphDataStore(
    useShallow((s) => {
      const pinIds = s.nodePins[nodeId];
      if (!pinIds?.length) return EMPTY_PINS;
      return pinIds.map((pid) => s.pins[pid]).filter(Boolean);
    }),
  );
  const pinConns = useGraphDataStore(
    useShallow((s) => {
      const pinIds = s.nodePins[nodeId];
      if (!pinIds?.length) return EMPTY_PIN_CONNECTIONS;
      return pinIds.map((pid) => s.pinConnections[pid] ?? EMPTY_CONNECTIONS);
    }),
  );
  const nodeType = node?.nodeType;
  const definition = useNodeRegistryStore((s) =>
    nodeType ? s.definitions.get(nodeType) : undefined,
  );

  const pins = useMemo<PinView[]>(
    () =>
      pinObjs.map((pin, index) => ({
        ...pin,
        links: derivePinLinks(pin.id, pinConns[index]),
      })),
    [pinObjs, pinConns],
  );

  const pinSpecs = useMemo(
    () => resolveNodePinSpecs(nodeId, pins, definition),
    [nodeId, pins, definition],
  );

  const documentation = useMemo(() => {
    const meta = getNodeDefinitionMeta(definition);
    return resolveNodeDocumentationContent(meta, i18n.language, node?.description);
  }, [definition, node?.description, i18n.language]);

  if (!node) {
    return (
      <DetailPanelShell title={t('detail.titleNode')}>
        <DetailText as="div" tone="muted" className="p-4">
          {t('detail.nodeNotFound')}
        </DetailText>
      </DetailPanelShell>
    );
  }

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: node.title || node.nodeType })}>
      <DetailForm>
        <DetailReadonlyField label={t('detail.fields.name')} tone="body" className="font-medium">
          {node.title}
        </DetailReadonlyField>
        {node.category?.length > 0 && (
          <DetailReadonlyField label={t('detail.fields.category')}>
            {node.category.join(' / ')}
          </DetailReadonlyField>
        )}
      </DetailForm>
      <NodePinInterfacePanel inputs={pinSpecs.inputs} outputs={pinSpecs.outputs} />
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
}
