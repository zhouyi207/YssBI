import type { DeepReadonly } from "@/shared/types/deepReadonly";
import {
  outputPinRef,
  resultRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
export { outputPinRef, resultRef };
export type { InspectableResultRef };
import type { ResultDescriptor } from "./types";
import type {
  ResultQueryCoordinator,
  ResultQueryReadCapability,
  ResultQueryOutcome,
} from "./resultQueryCoordinator";

export interface InspectableResultQueryDependencies {
  readonly coordinator: ResultQueryCoordinator;
  readonly read: ResultQueryReadCapability;
}

export interface ResolvedInspectableResultRef {
  readonly ref: Extract<InspectableResultRef, { kind: "result" }> | null;
  readonly status: ResultQueryOutcome["status"];
}

export async function resolveInspectableResultRef(
  ref: InspectableResultRef,
  dependencies: InspectableResultQueryDependencies,
): Promise<ResolvedInspectableResultRef> {
  if (ref.kind === "result") {
    return { ref, status: "published" };
  }

  const request = { graphPath: ref.graphPath, output: ref.output };
  const status = await dependencies.coordinator.loadPinResult(request);
  if (status.status !== "published") {
    return { ref: null, status: status.status };
  }

  const result = dependencies.read.getPinResult(request);
  return {
    ref: result ? { kind: "result", resultId: result.resultId } : null,
    status: result ? "published" : "notReady",
  };
}

export async function resolveInspectableResult(
  ref: InspectableResultRef,
  dependencies: InspectableResultQueryDependencies,
): Promise<DeepReadonly<ResultDescriptor> | null> {
  const resolved = await resolveInspectableResultRef(ref, dependencies);
  if (!resolved.ref) return null;

  const status = await dependencies.coordinator.loadDescriptor({
    resultId: resolved.ref.resultId,
  });
  if (status.status !== "published") return null;
  return dependencies.read.getDescriptor(resolved.ref.resultId);
}
