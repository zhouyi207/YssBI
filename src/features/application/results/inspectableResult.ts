import {
  outputPinRef,
  resultRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
export { outputPinRef, resultRef };
export type { InspectableResultRef };
import { resultReference, type ResultReference } from "@/shared/types/domain/result";
import type { ResultQueryCoordinator, ResultQueryReadCapability } from "./resultQueryCoordinator";

export interface InspectableResultQueryDependencies {
  readonly coordinator: Pick<ResultQueryCoordinator, "loadPinResult">;
  readonly read: Pick<ResultQueryReadCapability, "getPinResult">;
}

export async function resolveInspectableResultRef(
  ref: InspectableResultRef,
  dependencies: InspectableResultQueryDependencies,
): Promise<ResultReference | null> {
  if (ref.kind === "result") {
    return resultReference(ref);
  }

  const request = { graphPath: ref.graphPath, output: ref.output };
  const status = await dependencies.coordinator.loadPinResult(request);
  if (status.status !== "published") {
    return null;
  }

  const result = dependencies.read.getPinResult(request);
  return result ? resultReference(result) : null;
}
