import { useMemo } from "react";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";

export function useGraphDiagnosticCounts(): Record<string, number> {
  const graphEntities = useGraphProjectionStore((state) => state.graphEntities);

  return useMemo(
    () =>
      Object.fromEntries(
        Object.entries(graphEntities).flatMap(([graphPath, bucket]) =>
          bucket.diagnostics.length > 0 ? [[graphPath, bucket.diagnostics.length]] : [],
        ),
      ),
    [graphEntities],
  );
}
