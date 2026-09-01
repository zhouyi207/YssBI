import type { TFunction } from "i18next";
import { portAddressKey } from "@/features/domain/editorProjection";
import {
  outputPinRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
import type {
  EditorConnectionProjectionDto,
  PortAddressDto,
} from "@/shared/types/domain/editorProjection";

export type PinViewDisabledReason = "exec_pin" | "not_applicable" | "no_run" | "no_upstream";

export interface ResolvePinViewTargetParams {
  graphPath: string;
  address?: PortAddressDto;
  direction: "input" | "output";
  isExec: boolean;
  connections?: readonly EditorConnectionProjectionDto[];
}

export interface PinViewUiState {
  showMenu: boolean;
  enabled: boolean;
  disabledReason: PinViewDisabledReason | null;
  refs: InspectableResultRef[];
}

function sameAddress(left: PortAddressDto, right: PortAddressDto): boolean {
  return portAddressKey(left) === portAddressKey(right);
}

export function resolveUpstreamOutputs(
  input: PortAddressDto,
  connections: readonly EditorConnectionProjectionDto[] | undefined,
): PortAddressDto[] {
  return (connections ?? [])
    .filter((connection) => sameAddress(connection.input, input))
    .map((connection) => connection.output);
}

export function inspectableRefsFromPinView(
  params: ResolvePinViewTargetParams,
): InspectableResultRef[] {
  const { graphPath, address, direction, isExec, connections } = params;
  if (isExec || !address) return [];
  const outputs = direction === "output" ? [address] : resolveUpstreamOutputs(address, connections);
  return outputs.map((output) => outputPinRef(graphPath, output));
}

export function evaluatePinViewState(params: ResolvePinViewTargetParams): PinViewUiState {
  if (params.isExec) {
    return { showMenu: false, enabled: false, disabledReason: "exec_pin", refs: [] };
  }

  const refs = inspectableRefsFromPinView(params);
  if (params.direction === "input" && refs.length === 0) {
    return {
      showMenu: false,
      enabled: false,
      disabledReason: "not_applicable",
      refs,
    };
  }

  if (refs.length === 0) {
    return { showMenu: true, enabled: false, disabledReason: "no_run", refs };
  }

  return { showMenu: true, enabled: true, disabledReason: null, refs };
}

export function pinViewDisabledTitle(
  reason: PinViewDisabledReason | null,
  t: TFunction,
): string | undefined {
  if (!reason || reason === "exec_pin" || reason === "not_applicable") return undefined;
  const key = {
    no_run: "contextMenu.pin.viewDisabledNoRun",
    no_upstream: "contextMenu.pin.viewDisabledNoUpstream",
  }[reason];
  return t(key);
}

export function buildPinViewParams(input: ResolvePinViewTargetParams): ResolvePinViewTargetParams {
  return input;
}
