import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useGraphRead } from "@/features/core/graph/read";
import type {
  ConnectionData,
  NodeData,
  PinData,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { NodePinViewModel } from "./NodePinViewModel";
import { detailEmptyHintClass } from "../shared/detailStyles";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { NodePinConnectionField } from "./NodePinConnectionField";

interface NodePinInterfacePanelProps {
  graphPath: string;
  inputs: NodePinViewModel[];
  outputs: NodePinViewModel[];
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
  pins: NodePinViewModel[];
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

export function NodePinInterfacePanel({ graphPath, inputs, outputs }: NodePinInterfacePanelProps) {
  const { t } = useTranslation();
  const bucket = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]);
  const graphPins = useMemo(
    () => Object.values(bucket?.pins ?? {}).map((pin) => structuredClone(pin) as PinData),
    [bucket],
  );
  const graphConnections = useMemo(
    () =>
      Object.values(bucket?.connections ?? {}).map(
        (connection) => structuredClone(connection) as ConnectionData,
      ),
    [bucket],
  );
  const graphNodes = useMemo(
    () => structuredClone(bucket?.nodes ?? {}) as Record<string, NodeData>,
    [bucket],
  );

  return (
    <>
      <DetailCollapsibleSection title={t("detail.nodeDoc.inputs")}>
        <PinList
          graphPath={graphPath}
          emptyLabel={t("detail.nodeDoc.noInputs")}
          pins={inputs}
          graphPins={graphPins}
          nodes={graphNodes}
          connections={graphConnections}
        />
      </DetailCollapsibleSection>
      <DetailCollapsibleSection title={t("detail.nodeDoc.outputs")}>
        <PinList
          graphPath={graphPath}
          emptyLabel={t("detail.nodeDoc.noOutputs")}
          pins={outputs}
          graphPins={graphPins}
          nodes={graphNodes}
          connections={graphConnections}
        />
      </DetailCollapsibleSection>
    </>
  );
}
