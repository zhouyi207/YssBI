import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useGraphDraftEditingLocked } from "@/features/application/graphDraft/useGraphDraftEditingLocked";
import { useGraphRead } from "@/features/core/graph/read";
import type {
  ConnectionData,
  NodeData,
  PinData,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { PortInstanceAdditionDto } from "@/shared/types/domain/editorProjection";
import type { NodePinViewModel } from "./NodePinViewModel";
import { detailEmptyHintClass } from "../shared/detailStyles";
import { DetailCollapsibleSection } from "../shared/DetailCollapsibleSection";
import { NodePinConnectionField } from "./NodePinConnectionField";
import {
  AddNodePortInstanceButton,
  RemoveNodePortInstanceButton,
} from "./NodePortInstanceControls";

interface NodePinInterfacePanelProps {
  graphPath: string;
  nodeId: string;
  inputs: NodePinViewModel[];
  outputs: NodePinViewModel[];
  portInstanceAdditions: readonly PortInstanceAdditionDto[];
}

function PinList({
  graphPath,
  nodeId,
  emptyLabel,
  pins,
  graphPins,
  nodes,
  connections,
  additions,
  disabled,
}: {
  graphPath: string;
  nodeId: string;
  emptyLabel: string;
  pins: NodePinViewModel[];
  graphPins: readonly PinData[];
  nodes: Readonly<Record<string, NodeData>>;
  connections: readonly ConnectionData[];
  additions: readonly PortInstanceAdditionDto[];
  disabled: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      {pins.map((pin) => {
        const pinData = graphPins.find((candidate) => candidate.id === pin.id);
        return (
          <div key={`${graphPath}-${pin.id}`} className="flex flex-col gap-1">
            <NodePinConnectionField
              graphPath={graphPath}
              pin={pin}
              pinData={pinData}
              pins={graphPins}
              nodes={nodes}
              connections={connections}
              disabled={disabled}
            />
            {pinData ? (
              <RemoveNodePortInstanceButton
                graphPath={graphPath}
                pin={pinData}
                disabled={disabled}
              />
            ) : null}
          </div>
        );
      })}
      {pins.length === 0 && additions.length === 0 ? (
        <div className={detailEmptyHintClass}>{emptyLabel}</div>
      ) : null}
      {additions.length > 0 ? (
        <div className="flex flex-col items-end gap-1 border-t border-border/50 px-1 pt-2">
          {additions.map((addition) => (
            <AddNodePortInstanceButton
              key={addition.templateKey}
              graphPath={graphPath}
              nodeId={nodeId}
              addition={addition}
              disabled={disabled}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function NodePinInterfacePanel({
  graphPath,
  nodeId,
  inputs,
  outputs,
  portInstanceAdditions,
}: NodePinInterfacePanelProps) {
  const { t } = useTranslation();
  const bucket = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]);
  const editingLocked = useGraphDraftEditingLocked(graphPath);
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
          nodeId={nodeId}
          emptyLabel={t("detail.nodeDoc.noInputs")}
          pins={inputs}
          graphPins={graphPins}
          nodes={graphNodes}
          connections={graphConnections}
          additions={portInstanceAdditions.filter((addition) => addition.direction === "input")}
          disabled={editingLocked}
        />
      </DetailCollapsibleSection>
      <DetailCollapsibleSection title={t("detail.nodeDoc.outputs")}>
        <PinList
          graphPath={graphPath}
          nodeId={nodeId}
          emptyLabel={t("detail.nodeDoc.noOutputs")}
          pins={outputs}
          graphPins={graphPins}
          nodes={graphNodes}
          connections={graphConnections}
          additions={portInstanceAdditions.filter((addition) => addition.direction === "output")}
          disabled={editingLocked}
        />
      </DetailCollapsibleSection>
    </>
  );
}
