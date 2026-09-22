import { resultQueryCoordinator, resultQueryRead } from "@/features/application/results/runtime";
import {
  isResultReference,
  resultReference,
  type ResultReference,
} from "@/shared/types/domain/result";
import {
  type ResultDescriptor,
  type ResultPlotKind,
  type ResultReportKind,
} from "@/shared/types/domain/result";
import { logger } from "@/features/application/observability/appLogger";

export type PresentationWindowState =
  | { status: "loading" }
  | { status: "missing_result_id" }
  | { status: "not_found" }
  | { status: "load_failed" }
  | { status: "ready"; descriptor: ResultDescriptor; payload: PresentationPayload };

export type PresentationPayload =
  | { mode: "inspector"; descriptor: ResultDescriptor }
  | { mode: "plot"; chart: ResultPlotKind; data: unknown }
  | { mode: "report"; report: ResultReportKind; data: unknown };

async function loadReadyPayload(descriptor: ResultDescriptor): Promise<PresentationPayload> {
  if (descriptor.presentation.kind === "inspector") {
    return { mode: "inspector", descriptor };
  }

  if (descriptor.valueKind === "scalar") {
    const reference = resultReference(descriptor);
    const outcome = await resultQueryCoordinator.loadValue(reference);
    if (outcome.status !== "published") throw new Error("Result value unavailable");
    const value = resultQueryRead.getValue(reference);
    if (!value || value.kind !== "value") {
      throw new Error("Presentation results require a canonical scalar value");
    }
    return descriptor.presentation.kind === "plot"
      ? { mode: "plot", chart: descriptor.presentation.chart, data: value.value }
      : { mode: "report", report: descriptor.presentation.report, data: value.value };
  }

  throw new Error("Plot and report presentations require a complete scalar payload");
}

export async function loadPresentationWindow(
  reference: ResultReference,
): Promise<PresentationWindowState> {
  if (!isResultReference(reference)) return { status: "missing_result_id" };
  try {
    const outcome = await resultQueryCoordinator.loadDescriptor(reference);
    if (outcome.status === "failed" || outcome.status === "stale")
      throw new Error("Result descriptor unavailable");
    const cached = resultQueryRead.getDescriptor(reference);
    const descriptor = cached ? (structuredClone(cached) as ResultDescriptor) : null;
    if (!descriptor) return { status: "not_found" };
    return { status: "ready", descriptor, payload: await loadReadyPayload(descriptor) };
  } catch (error) {
    logger.app.error(
      `Failed to load presentation result: ${error instanceof Error ? error.message : String(error)}`,
      "loadPresentationWindow",
    );
    return { status: "load_failed" };
  }
}
