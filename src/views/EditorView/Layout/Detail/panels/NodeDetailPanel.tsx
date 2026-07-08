import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import type { PinData, PinView } from '@/shared/types/store/graph';
import { getNodeDefinitionMeta } from '@/shared/types/domain/node';
import { CALL_FUNCTION_NODE_TYPE, resolveEffectiveDefinition } from '@/features/domain/nodeDefinition';
import { openGraphResource, resolveGraphResourceMeta } from '@/features/application/editor/openGraphResource';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { derivePinConnectionView } from '@/features/core/dataStore/pinLinks';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { useExecutionStore } from '@/features/core/execution';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import { resolveNodeDocumentationContent } from '../nodeDocumentation';
import { resolveNodePinSpecs } from '../resolveNodePinSpecs';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailText } from '../shared/DetailText';
import type { PinResultState } from '@/shared/types/ui';

const EMPTY_PINS: PinData[] = [];
const EMPTY_PIN_CONNECTIONS: string[][] = [];

function isPresent<T>(value: T | null | undefined): value is T {
  return value != null;
}

interface NodeDetailPanelProps {
  nodeId: string;
}

export function NodeDetailPanel({ nodeId }: NodeDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const graphPath = useGraphDataStore((s) => {
    for (const [gid, bucket] of Object.entries(s.graphEntities)) {
      if (bucket.nodes[nodeId]) return gid;
    }
    return undefined;
  });
  const node = useGraphDataStore((s) => (graphPath ? s.getGraphNode(graphPath, nodeId) : undefined));
  const pinObjs = useGraphDataStore(
    useShallow((s) => {
      if (!graphPath) return EMPTY_PINS;
      const pinIds = s.getGraphNodePins(graphPath, nodeId);
      if (!pinIds.length) return EMPTY_PINS;
      return pinIds.map((pid) => s.getGraphPin(graphPath, pid)).filter(isPresent);
    }),
  );
  const pinConns = useGraphDataStore(
    useShallow((s) => {
      if (!graphPath) return EMPTY_PIN_CONNECTIONS;
      const pinIds = s.getGraphNodePins(graphPath, nodeId);
      if (!pinIds.length) return EMPTY_PIN_CONNECTIONS;
      return pinIds.map((pid) => s.getGraphPinConnections(graphPath, pid));
    }),
  );
  const nodeType = node?.nodeType;
  const definition = useNodeRegistryStore((s) =>
    nodeType ? s.definitions.get(nodeType) : undefined,
  );
  const effectiveDefinition = useMemo(() => {
    if (!definition) return undefined;
    if (nodeType !== CALL_FUNCTION_NODE_TYPE || !node?.subGraphPath) return definition;
    return resolveEffectiveDefinition(definition, { subGraphPath: node.subGraphPath });
  }, [definition, nodeType, node?.subGraphPath]);

  const pins = useMemo<PinView[]>(
    () =>
      pinObjs.map((pin, index) => ({
        ...pin,
        ...derivePinConnectionView(pinConns[index]),
      })),
    [pinObjs, pinConns],
  );

  const pinSpecs = useMemo(
    () => resolveNodePinSpecs(nodeId, pins, effectiveDefinition),
    [nodeId, pins, effectiveDefinition],
  );

  const executionGraph = useExecutionStore((s) => (graphPath ? s.graphs[graphPath] : undefined));
  const pinResults = useMemo(() => {
    if (!executionGraph) return new Map<string, PinResultState>();
    return new Map(executionGraph.pinResults);
  }, [executionGraph]);

  const documentation = useMemo(() => {
    const meta = getNodeDefinitionMeta(effectiveDefinition);
    return resolveNodeDocumentationContent(meta, i18n.language, node?.description);
  }, [effectiveDefinition, node?.description, i18n.language]);

  if (!node || !graphPath) {
    return (
      <DetailPanelShell title={t('detail.titleNode')}>
        <DetailText as="div" tone="muted" className="p-4">
          {t('detail.nodeNotFound')}
        </DetailText>
      </DetailPanelShell>
    );
  }

  const callTargetMissing =
    nodeType === CALL_FUNCTION_NODE_TYPE &&
    !!node.subGraphPath &&
    !resolveGraphResourceMeta(node.subGraphPath);

  const handleOpenCallTarget = () => {
    if (!node.subGraphPath) return;
    void openGraphResource(node.subGraphPath, 'function');
  };

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
        {nodeType === CALL_FUNCTION_NODE_TYPE && node.subGraphPath && (
          <DetailReadonlyField label={t('detail.fields.graph')}>
            <button
              type="button"
              className="text-left text-[var(--accent-color)] hover:underline"
              onClick={handleOpenCallTarget}
            >
              {callTargetMissing
                ? t('detail.callFunction.missingTarget', { path: node.subGraphPath })
                : t('detail.callFunction.openTarget')}
            </button>
          </DetailReadonlyField>
        )}
      </DetailForm>
      <NodePinInterfacePanel
        graphPath={graphPath}
        inputs={pinSpecs.inputs}
        outputs={pinSpecs.outputs}
        pinResults={pinResults}
        executionStatus={executionGraph?.status}
      />
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
}
