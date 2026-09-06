import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { readPinResultStatus, resultQueryRead } from "@/features/application/results";
import type { GraphOutputRefDto } from "@/shared/types/domain/executionDemand";
import { ResultContent } from "./ResultContent";

export function ResultPanel({
  resultId,
  source,
}: {
  readonly resultId: string;
  readonly source?: GraphOutputRefDto | null;
}) {
  const { t } = useTranslation();
  const request = source ? { graphPath: source.graphPath, output: source.port } : null;
  const currentId = useSyncExternalStore(resultQueryRead.subscribe, () =>
    request ? (resultQueryRead.getPinResult(request)?.resultId ?? null) : resultId,
  );
  const status = useSyncExternalStore(resultQueryRead.subscribe, () =>
    request ? readPinResultStatus(request) : "unavailable",
  );
  return (
    <div
      className="flex h-full min-h-0 flex-col overflow-hidden bg-background"
      data-workbench-result-panel
    >
      {currentId ? (
        <ResultContent key={currentId} resultId={currentId} />
      ) : (
        <div className="flex flex-1 items-center justify-center p-4 text-sm text-muted-foreground">
          {t(
            status === "running"
              ? "common.loading"
              : status === "failed"
                ? "resultState.executionFailed"
                : status === "cancelled"
                  ? "resultState.cancelled"
                  : "sourceInspector.noSource",
          )}
        </div>
      )}
    </div>
  );
}
