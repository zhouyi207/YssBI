import type { ReactNode } from "react";
import type { ResultDescriptor } from "../types";
import { resolveResultRenderer } from "../resolveRenderer";
import {
  JsonResultView,
  ScalarResultView,
  DataSeriesResultView,
  SequenceResultView,
} from "./renderers/ResultRenderers";

export interface UnifiedResultViewProps {
  payload: ResultDescriptor;
  renderInfo?: (payload: ResultDescriptor) => ReactNode;
}

export function UnifiedResultView({ payload, renderInfo }: UnifiedResultViewProps) {
  const kind = resolveResultRenderer(payload);

  switch (kind) {
    case "sequence":
      return <SequenceResultView payload={payload} />;
    case "dataseries":
      return <DataSeriesResultView payload={payload} />;
    case "scalar":
      return <ScalarResultView payload={payload} />;

    case "info":
      return renderInfo?.(payload) ?? null;
    case "json":
      return <JsonResultView payload={payload} />;
    default:
      return <JsonResultView payload={payload} />;
  }
}
