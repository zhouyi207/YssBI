import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useGraphDataStore } from '@/features/application/viewCapabilities';
import type { ConnectionData, NodeData, PinData } from '@/shared/types/store/graph';
import type { ResolvedPinSpec } from '../resolveNodePinSpecs';
import { detailEmptyHintClass } from '../shared/detailStyles';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';
import { NodePinConnectionField } from './NodePinConnectionField';

interface NodePinInterfacePanelProps {
  graphPath: string;
  inputs: ResolvedPinSpec[];
  outputs: ResolvedPinSpec[];
}

function PinList({
  graphPath,
  emptyLabel,
  pins,
  graphPins,
  nodes,
  connections,
}: {
  graphPath: string;
  emptyLabel: string;
  pins: ResolvedPinSpec[];
  graphPins: readonly PinData[];
  nodes: Readonly<Record<string, NodeData>>;
  connections: readonly ConnectionData[];
}) {
  return (
    <div className="flex flex-col gap-1">
      {pins.length > 0 ? (
        pins.map((pin) => (
          <NodePinConnectionField
            key={`${graphPath}-${pin.id}`}
            graphPath={graphPath}
            pin={pin}
            pinData={graphPins.find((candidate) => candidate.id === pin.id)}
            pins={graphPins}
            nodes={nodes}
            connections={connections}
          />
        ))
      ) : (
        <div className={detailEmptyHintClass}>{emptyLabel}</div>
      )}
    </div>
  );
}

export function NodePinInterfacePanel({
  graphPath,
  inputs,
  outputs,
}: NodePinInterfacePanelProps) {
  const { t } = useTranslation();
  const bucket = useGraphDataStore((state) => state.graphEntities[graphPath]);
  const graphPins = useMemo(() => Object.values(bucket?.pins ?? {}), [bucket]);
  const graphConnections = useMemo(() => Object.values(bucket?.connections ?? {}), [bucket]);
  const graphNodes = bucket?.nodes ?? {};

  return (
    <>
      <DetailCollapsibleSection
        title={t('detail.nodeDoc.inputs')}
      >
        <PinList
          graphPath={graphPath}
          emptyLabel={t('detail.nodeDoc.noInputs')}
          pins={inputs}
          graphPins={graphPins}
          nodes={graphNodes}
          connections={graphConnections}
        />
      </DetailCollapsibleSection>
      <DetailCollapsibleSection
        title={t('detail.nodeDoc.outputs')}
      >
        <PinList
          graphPath={graphPath}
          emptyLabel={t('detail.nodeDoc.noOutputs')}
          pins={outputs}
          graphPins={graphPins}
          nodes={graphNodes}
          connections={graphConnections}
        />
      </DetailCollapsibleSection>
    </>
  );
}
