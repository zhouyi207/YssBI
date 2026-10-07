import { useReadProjection } from "@/features/core/state/readProjection";
import type { LogDomainId } from "@/features/domain/log/logDomains";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { logBuffer, type LogSnapshot } from "./logBuffer";
import { applyLogFilter, useLogStore } from "./logStore";

export function useLiveLogs<T>(selector: (snapshot: LogSnapshot) => T): T {
  return useReadProjection(logBuffer, selector);
}

export function useFilteredLogs<T>(
  domain: LogDomainId,
  selector: (entries: readonly LogRecordDto[]) => T,
): T {
  const filter = useLogStore((state) => state.filter);
  return useLiveLogs((snapshot) =>
    selector(applyLogFilter(snapshot.entriesByDomain[domain], filter, domain)),
  );
}
