import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import type { PinData, PinView } from '@/shared/types/store/graph';
import { getNodeDefinitionMeta } from '@/shared/types/domain/node';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { derivePinConnectionView } from '@/features/core/dataStore/pinLinks';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { useExecutionStore } from '@/features/core/execution';
import { UnifiedDataView } from '@/features/core/dataView';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import { resolveNodeDocumentationContent } from '../nodeDocumentation';
import { resolveNodePinSpecs } from '../resolveNodePinSpecs';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailText } from '../shared/DetailText';
import type { PinResultState } from '@/shared/types/ui';

const EMPTY_PINS: PinData[] = [];
const EMPTY_CONNECTIONS: string[] = [];
const EMPTY_PIN_CONNECTIONS: string[][] = [];

function isPresent<T>(value: T | null | undefined): value is T {
  return value != null;
}

interface NodeDetailPanelProps {
  nodeId: string;
}

export function NodeDetailPanel({ nodeId }: NodeDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const graphId = useGraphDataStore((s) => {
    for (const [gid, bucket] of Object.entries(s.graphEntities)) {
      if (bucket.nodes[nodeId]) return gid;
    }
    return s.nodes[nodeId]?.graphId;
  });
  const node = useGraphDataStore((s) =>
    graphId ? s.getGraphNode(graphId, nodeId) : s.nodes[nodeId],
  );
  const pinObjs = useGraphDataStore(
    useShallow((s) => {
      const pinIds = graphId ? s.getGraphNodePins(graphId, nodeId) : s.nodePins[nodeId];
      if (!pinIds?.length) return EMPTY_PINS;
      return pinIds.map((pid) => (graphId ? s.getGraphPin(graphId, pid) : s.pins[pid])).filter(isPresent);
    }),
  );
  const pinConns = useGraphDataStore(
    useShallow((s) => {
      const pinIds = graphId ? s.getGraphNodePins(graphId, nodeId) : s.nodePins[nodeId];
      if (!pinIds?.length) return EMPTY_PIN_CONNECTIONS;
      return pinIds.map((pid) =>
        graphId ? s.getGraphPinConnections(graphId, pid) : s.pinConnections[pid] ?? EMPTY_CONNECTIONS,
      );
    }),
  );
  const nodeType = node?.nodeType;
  const [selectedResultPinId, setSelectedResultPinId] = useState<string | null>(null);
  const definition = useNodeRegistryStore((s) =>
    nodeType ? s.definitions.get(nodeType) : undefined,
  );

  const pins = useMemo<PinView[]>(
    () =>
      pinObjs.map((pin, index) => ({
        ...pin,
        ...derivePinConnectionView(pinConns[index]),
      })),
    [pinObjs, pinConns],
  );

  const pinSpecs = useMemo(
    () => resolveNodePinSpecs(nodeId, pins, definition),
    [nodeId, pins, definition],
  );
  const executionGraphs = useExecutionStore((s) => s.graphs);
  const pinResults = useMemo(() => {
    const result = new Map<string, PinResultState>();
    for (const graph of Object.values(executionGraphs)) {
      for (const [pinId, pinResult] of graph.pinResults) {
        if (pinResult.nodeId === nodeId) result.set(pinId, pinResult);
      }
    }
    return result;
  }, [executionGraphs, nodeId]);
  const selectedResult = selectedResultPinId ? pinResults.get(selectedResultPinId) : null;

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
      <NodePinInterfacePanel
        inputs={pinSpecs.inputs}
        outputs={pinSpecs.outputs}
        pinResults={pinResults}
        selectedResultPinId={selectedResultPinId}
        onInspectResult={setSelectedResultPinId}
      />
      {selectedResult ? <UnifiedDataView payload={selectedResult.descriptor} /> : null}
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
}
