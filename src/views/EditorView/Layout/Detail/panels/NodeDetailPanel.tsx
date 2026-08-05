import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import type { GraphEntitiesState } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { derivePinConnectionView } from '@/features/core/dataStore/pinLinks';
import {
  executionStatusForSourceGraph,
  pinResultsForSourceGraph,
  useExecutionStore,
} from '@/features/core/execution';
import type { PinData, PinView } from '@/shared/types/store/graph';
import type { PinResultState } from '@/shared/types/ui';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { DetailForm, DetailReadonlyField } from '../shared/DetailForm';
import { DetailBadge, DetailText } from '../shared/DetailText';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import { NodeParameterEditor } from '../node/parameterEditors/NodeParameterEditor';

const EMPTY_PINS: PinData[] = [];
const EMPTY_PIN_CONNECTIONS: string[][] = [];

function isPresent<T>(value: T | null | undefined): value is T {
  return value != null;
}

function formatProjectedValue(value: unknown): string {
  if (value == null) return '—';
  if (typeof value === 'string') return value;
  return JSON.stringify(value);
}

export function selectNodeDetailNode(
  state: GraphEntitiesState,
  graphPath: string,
  nodeId: string,
) {
  return state.graphEntities[graphPath]?.nodes[nodeId];
}

interface NodeDetailPanelProps {
  graphPath: string;
  nodeId: string;
}

export function NodeDetailPanel({ graphPath, nodeId }: NodeDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const node = useGraphDataStore((state) => selectNodeDetailNode(state, graphPath, nodeId));
  const pinObjs = useGraphDataStore(
    useShallow((state) => {
      const bucket = state.graphEntities[graphPath];
      const pinIds = bucket?.nodePins[nodeId];
      if (!bucket || !pinIds?.length) return EMPTY_PINS;
      return pinIds.map((pinId) => bucket.pins[pinId]).filter(isPresent);
    }),
  );
  const pinConns = useGraphDataStore(
    useShallow((state) => {
      const bucket = state.graphEntities[graphPath];
      const pinIds = bucket?.nodePins[nodeId];
      if (!bucket || !pinIds?.length) return EMPTY_PIN_CONNECTIONS;
      return pinIds.map((pinId) => bucket.pinConnections[pinId]);
    }),
  );

  const pins = useMemo<PinView[]>(
    () =>
      pinObjs.map((pin, index) => ({
        ...pin,
        ...derivePinConnectionView(pinConns[index]),
      })),
    [pinObjs, pinConns],
  );

  const pinSpecs = useMemo(() => {
    const toSpec = (pin: PinView): ResolvedPinSpec => ({
      id: pin.id,
      name: pin.display?.instanceLabel ?? pin.display?.label ?? pin.name,
      direction: pin.direction,
      kind: pin.type === 'exec' ? 'Exec' : 'Data',
      typeLabel: pin.resolvedType?.display ?? pin.type,
      optional: false,
      slotKind:
        pin.instanceKind === 'userCreated'
          ? 'repeatable'
          : pin.instanceKind === 'derived'
            ? 'derivedFromInput'
            : 'fixed',
      connected: pin.connected,
      connectionIds: pin.connectionIds,
    });
    return {
      inputs: pins.filter((pin) => pin.direction === 'input').map(toSpec),
      outputs: pins.filter((pin) => pin.direction === 'output').map(toSpec),
    };
  }, [pins]);

  const executionGraphs = useExecutionStore((state) => state.graphs);
  const pinResults = useMemo<Map<string, PinResultState>>(
    () => pinResultsForSourceGraph(executionGraphs, graphPath),
    [executionGraphs, graphPath],
  );
  const executionStatus = useMemo(
    () => executionStatusForSourceGraph(executionGraphs, graphPath),
    [executionGraphs, graphPath],
  );

  if (!node) {
    return (
      <DetailPanelShell title={t('detail.titleNode')}>
        <DetailText as="div" tone="muted" className="p-4">
          {t('detail.nodeNotFound')}
        </DetailText>
      </DetailPanelShell>
    );
  }

  const documentation = node.display?.description ?? node.description;

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: node.title || node.nodeType })}>
      <DetailForm>
        <DetailReadonlyField
          label={t('detail.fields.name')}
          tone="body"
          valueClassName="min-w-0"
          className="min-w-0 truncate font-medium"
        >
          {node.display?.title ?? node.title}
        </DetailReadonlyField>
      </DetailForm>
      {node.parameterEditors && node.parameterEditors.length > 0 && (
        <DetailCollapsibleSection title={t('detail.parameters')} defaultOpen>
          <DetailForm>
            {node.parameterEditors.map((parameter) => (
              <NodeParameterEditor
                key={parameter.key}
                graphPath={graphPath}
                nodeId={nodeId}
                locale={i18n.language}
                parameter={parameter}
                diagnostics={node.diagnostics ?? []}
                formatFallback={formatProjectedValue}
              />
            ))}
          </DetailForm>
        </DetailCollapsibleSection>
      )}
      {node.capabilities && (
        <DetailCollapsibleSection title="Capabilities">
          <div className="flex flex-wrap gap-1.5 px-1 py-2">
            {Object.entries(node.capabilities)
              .filter(([, enabled]) => enabled)
              .map(([capability]) => (
                <DetailBadge key={capability}>{capability}</DetailBadge>
              ))}
          </div>
        </DetailCollapsibleSection>
      )}
      {node.diagnostics && node.diagnostics.length > 0 && (
        <DetailCollapsibleSection title="Diagnostics" defaultOpen>
          <div className="space-y-2 px-1 py-2">
            {node.diagnostics.map((diagnostic, index) => (
              <div key={`${diagnostic.code}-${index}`} className="flex items-start gap-2">
                <DetailBadge>{diagnostic.severity}</DetailBadge>
                <DetailText as="span" tone="muted">
                  {diagnostic.message}
                </DetailText>
              </div>
            ))}
          </div>
        </DetailCollapsibleSection>
      )}
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
