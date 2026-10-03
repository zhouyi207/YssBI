import { useMemo } from "react";
import { useCurrentPinResult } from "./runtime";
import { useResultValue } from "./useResultValue";
import { descriptionColumns } from "./descriptionResult";

export function useDescriptionResult(graphPath: string, nodeId: string) {
  const descriptor = useCurrentPinResult({
    graphPath,
    output: { kind: "declared", nodeId, portKey: "result" },
  });
  const reference =
    descriptor?.presentation.kind === "report" && descriptor.presentation.report === "structured"
      ? descriptor
      : null;
  const query = useResultValue(reference);
  const columns = useMemo(
    () => (query.value?.kind === "value" ? descriptionColumns(query.value.value) : null),
    [query.value],
  );
  return {
    available: descriptor !== null,
    loading: query.loading || (reference !== null && query.value === null),
    invalid:
      (descriptor !== null && reference === null) || (query.value !== null && columns === null),
    error: query.error,
    reload: query.reload,
    columns,
  };
}
