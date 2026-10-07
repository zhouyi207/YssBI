import { memo } from "react";
import { useTranslation } from "react-i18next";
import { useGraphEditingLocked } from "@/features/application/graphEditing/useGraphEditingLocked";
import { useGraphRead } from "@/features/core/graph/read";
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

const PinRow = memo(function PinRow({
  graphPath,
  pin,
  disabled,
}: {
  graphPath: string;
  pin: NodePinViewModel;
  disabled: boolean;
}) {
  const pinData = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]?.pins[pin.id]);
  return (
    <div className="flex flex-col gap-1">
      <NodePinConnectionField
        graphPath={graphPath}
        pin={pin}
        pinData={pinData}
        disabled={disabled}
      />
      {pinData ? (
        <RemoveNodePortInstanceButton graphPath={graphPath} pin={pinData} disabled={disabled} />
      ) : null}
    </div>
  );
});

function PinList({
  graphPath,
  nodeId,
  emptyLabel,
  pins,
  additions,
  disabled,
}: {
  graphPath: string;
  nodeId: string;
  emptyLabel: string;
  pins: NodePinViewModel[];
  additions: readonly PortInstanceAdditionDto[];
  disabled: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      {pins.map((pin) => (
        <PinRow
          key={`${graphPath}-${pin.id}`}
          graphPath={graphPath}
          pin={pin}
          disabled={disabled}
        />
      ))}
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

  return (
    <>
      <DetailCollapsibleSection title={t("detail.nodeDoc.inputs")}>
        <PinList
          graphPath={graphPath}
          nodeId={nodeId}
          emptyLabel={t("detail.nodeDoc.noInputs")}
          pins={inputs}
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
          additions={portInstanceAdditions.filter((addition) => addition.direction === "output")}
          disabled={editingLocked}
        />
      </DetailCollapsibleSection>
    </>
  );
}
