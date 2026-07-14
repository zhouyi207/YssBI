import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import type { PinData, PinView } from '@/shared/types/store/graph';
import { getNodeDefinitionMeta } from '@/shared/types/domain/node';
import { CALL_FUNCTION_NODE_TYPE, resolveEffectiveDefinition } from '@/features/domain/nodeDefinition';
import { openGraphResource } from '@/features/application/editor/openGraphResource';
import { useCallFunctionIssue } from '@/features/application/graphDiagnostics/useCallFunctionDiagnostics';
import { updateCallFunctionTarget } from '@/features/application/graphDocument/graphDocumentActions';
import { useFunctionCatalog } from '@/features/core/editor/hooks/useFunctionCatalog';
import { uiStore } from '@/features/core/ui/UIStore';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { derivePinConnectionView } from '@/features/core/dataStore/pinLinks';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import {
  pinResultsForSourceGraph,
  executionStatusForSourceGraph,
  useExecutionStore,
} from '@/features/core/execution';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import { resolveNodeDocumentationContent } from '../nodeDocumentation';
import { resolveNodePinSpecs } from '../resolveNodePinSpecs';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import { VscGoToFile } from 'react-icons/vsc';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { DetailText } from '../shared/DetailText';
import { Select } from '@/shared/ui';
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
  const functionCatalog = useFunctionCatalog();
  const functionOptions = useMemo(
    () =>
      Object.values(functionCatalog)
        .map((entry) => ({ label: entry.name, value: entry.id }))
        .sort((a, b) => a.label.localeCompare(b.label)),
    [functionCatalog],
  );
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

  const executionGraphs = useExecutionStore((s) => s.graphs);
  const pinResults = useMemo(() => {
    if (!graphPath) return new Map<string, PinResultState>();
    return pinResultsForSourceGraph(executionGraphs, graphPath);
  }, [executionGraphs, graphPath]);
  const executionStatus = useMemo(
    () => (graphPath ? executionStatusForSourceGraph(executionGraphs, graphPath) : undefined),
    [executionGraphs, graphPath],
  );

  const documentation = useMemo(() => {
    const meta = getNodeDefinitionMeta(effectiveDefinition);
    return resolveNodeDocumentationContent(meta, i18n.language, node?.description);
  }, [effectiveDefinition, node?.description, i18n.language]);

  const callTargetIssue = useCallFunctionIssue(graphPath, nodeId);

  if (!node || !graphPath) {
    return (
      <DetailPanelShell title={t('detail.titleNode')}>
        <DetailText as="div" tone="muted" className="p-4">
          {t('detail.nodeNotFound')}
        </DetailText>
      </DetailPanelShell>
    );
  }

  const handleOpenCallTarget = () => {
    if (!node.subGraphPath) return;
    void openGraphResource(node.subGraphPath, 'function');
  };

  const handleCallTargetChange = (functionPath: string) => {
    if (!functionPath || functionPath === node.subGraphPath) return;
    void updateCallFunctionTarget(graphPath, nodeId, functionPath).catch((error) => {
      uiStore.showToast(formatErrorMessage(error), 'error');
    });
  };

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: node.title || node.nodeType })}>
      <DetailForm>
        <DetailReadonlyField
          label={t('detail.fields.name')}
          tone="body"
          valueClassName="min-w-0"
          className="min-w-0 truncate font-medium"
        >
          {node.title}
        </DetailReadonlyField>
        {node.category?.length > 0 && (
          <DetailReadonlyField
            label={t('detail.fields.category')}
            valueClassName="min-w-0"
            className="min-w-0 truncate"
          >
            {node.category.join(' / ')}
          </DetailReadonlyField>
        )}
        {nodeType === CALL_FUNCTION_NODE_TYPE && (
          <DetailFieldRow label={t('detail.callFunction.target')}>
            <div className="flex min-w-0 items-center gap-1.5">
              <div className="min-w-0 flex-1">
                <Select
                  className="w-full"
                  value={node.subGraphPath ?? ''}
                  options={functionOptions}
                  onChange={handleCallTargetChange}
                />
              </div>
              <ToolbarIconButton
                type="button"
                size="icon-sm"
                variant="outline"
                className="shrink-0"
                disabled={!node.subGraphPath || callTargetIssue != null}
                tooltip={
                  callTargetIssue?.kind === 'missing_target' && callTargetIssue.subGraphPath
                    ? t('detail.callFunction.missingTarget', { path: callTargetIssue.subGraphPath })
                    : callTargetIssue?.kind === 'empty_target'
                      ? t('graphDiagnostics.callFunctionEmptyTarget')
                      : t('detail.callFunction.openTarget')
                }
                onClick={handleOpenCallTarget}
              >
                <VscGoToFile size={14} />
              </ToolbarIconButton>
            </div>
          </DetailFieldRow>
        )}
      </DetailForm>
      <NodePinInterfacePanel
        graphPath={graphPath}
        inputs={pinSpecs.inputs}
        outputs={pinSpecs.outputs}
        pinResults={pinResults}
        executionStatus={executionStatus}
      />
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
}
