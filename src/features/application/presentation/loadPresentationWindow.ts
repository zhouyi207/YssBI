import { ResultService } from "@/services/result/resultService";
import {
  isResultReference,
  resultReference,
  type ResultReference,
} from "@/shared/types/domain/result";
import {
  isResultPlotKind,
  type ResultDescriptor,
  type ResultPlotKind,
  type ResultReportKind,
} from "@/shared/types/domain/result";
import { logger } from "@/features/application/observability/appLogger";
import { parsePlotChartFromLocation } from "./parsePresentationWindowQuery";

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

const PAGE_SIZE = 200;

function resolvePlotChart(descriptor: ResultDescriptor): ResultPlotKind {
  if (descriptor.presentation.kind === "plot") return descriptor.presentation.chart;
  const fallback = parsePlotChartFromLocation();
  return isResultPlotKind(fallback) ? fallback : "scatter";
}

async function loadReadyPayload(descriptor: ResultDescriptor): Promise<PresentationPayload> {
  if (descriptor.presentation.kind === "inspector") {
    return { mode: "inspector", descriptor };
  }

  if (descriptor.valueKind === "scalar") {
    const value = await ResultService.getValue(resultReference(descriptor));
    if (!value || value.kind !== "value") {
      throw new Error("Presentation results require a canonical scalar value");
    }
    return descriptor.presentation.kind === "plot"
      ? { mode: "plot", chart: resolvePlotChart(descriptor), data: value.value }
      : { mode: "report", report: descriptor.presentation.report, data: value.value };
  }

  const page = await ResultService.getPage(resultReference(descriptor), 0, PAGE_SIZE);
  if (!page) throw new Error("Result data was not found");
  if (descriptor.presentation.kind === "report") {
    throw new Error("Report results require a canonical scalar object");
  }
  return { mode: "plot", chart: resolvePlotChart(descriptor), data: page.values };
}

export async function loadPresentationWindow(
  reference: ResultReference,
): Promise<PresentationWindowState> {
  if (!isResultReference(reference)) return { status: "missing_result_id" };
  try {
    const descriptor = await ResultService.getDescriptor(reference);
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
