import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";
import { useGraphEditingLocked } from "@/features/application/graphEditing/useGraphEditingLocked";
import { useGraphRead } from "@/features/core/graph/read";
import type { ConnectionData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
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
  nodeTitles,
  connections,
  additions,
  disabled,
}: {
  graphPath: string;
  nodeId: string;
  emptyLabel: string;
  pins: NodePinViewModel[];
  graphPins: DeepReadonly<PinData[]>;
  nodeTitles: Readonly<Record<string, string>>;
  connections: DeepReadonly<ConnectionData[]>;
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
              nodeTitles={nodeTitles}
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
  const editingLocked = useGraphEditingLocked(graphPath);
  const graphPins = useGraphRead(
    useShallow((snapshot) => Object.values(snapshot.graphEntities[graphPath]?.pins ?? {})),
  );
  const graphConnections = useGraphRead(
    useShallow((snapshot) => Object.values(snapshot.graphEntities[graphPath]?.connections ?? {})),
  );
  const nodeTitles = useGraphRead(
    useShallow((snapshot) =>
      Object.fromEntries(
        Object.entries(snapshot.graphEntities[graphPath]?.nodes ?? {}).map(([id, node]) => [
          id,
          node.display.title,
        ]),
      ),
    ),
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
          nodeTitles={nodeTitles}
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
          nodeTitles={nodeTitles}
          connections={graphConnections}
          additions={portInstanceAdditions.filter((addition) => addition.direction === "output")}
          disabled={editingLocked}
        />
      </DetailCollapsibleSection>
    </>
  );
}
