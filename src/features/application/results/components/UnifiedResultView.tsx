import type { ResultDescriptor } from "../types";
import { ScalarResultView, SequenceResultView } from "./renderers/ResultRenderers";

/** Inspector payloads have already been routed by the presentation loader. */
export function UnifiedResultView({ payload }: { payload: ResultDescriptor }) {
  return payload.valueKind === "sequence" ? (
    <SequenceResultView payload={payload} />
  ) : (
    <ScalarResultView payload={payload} />
  );
}
