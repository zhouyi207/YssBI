import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { NodeData, PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { portAddressKey } from "./portAddressKey";

type NodeLabelSource = Pick<NodeData, "display">;
type PinLabelSource = Pick<PinData, "name" | "display">;

export interface NodePinDisplayBucket {
  readonly nodes?: Readonly<Record<string, NodeLabelSource>>;
  readonly pins?: Readonly<Record<string, PinLabelSource>>;
}

function nonEmpty(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

export function nodeDisplayTitle(node: NodeLabelSource | undefined): string | null {
  return nonEmpty(node?.display.title);
}

export function pinDisplayTitle(pin: PinLabelSource | undefined): string | null {
  return (
    nonEmpty(pin?.display.instanceLabel) ?? nonEmpty(pin?.display.label) ?? nonEmpty(pin?.name)
  );
}

export function formatNodePinDisplayLabel(
  nodeTitle: string | null | undefined,
  pinTitle: string | null | undefined,
): string | null {
  const node = nonEmpty(nodeTitle);
  const pin = nonEmpty(pinTitle);
  if (!node) return pin;
  if (!pin) return node;
  return `${node} · ${pin}`;
}

export function resolveNodePinDisplayLabel(
  bucket: NodePinDisplayBucket | undefined,
  address: PortAddressDto,
): string | null {
  return formatNodePinDisplayLabel(
    nodeDisplayTitle(bucket?.nodes?.[address.nodeId]),
    pinDisplayTitle(bucket?.pins?.[portAddressKey(address)]),
  );
}
