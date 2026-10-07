import { create } from "zustand";
import { LOG_LEVELS, type LogLevel, type LogRecordDto } from "@/shared/types/domain/log";
import type { LogDomainId } from "@/features/domain/log/logDomains";

export interface LogLogFilter {
  readonly levels: ReadonlySet<LogLevel>;
  readonly searchText: string;
}

export interface LogStore {
  filter: LogLogFilter;
  selectedLog: LogRecordDto | null;
  autoScroll: boolean;

  setSelectedLog: (log: LogRecordDto | null) => void;
  toggleLevel: (level: LogLevel) => void;
  setSearchText: (text: string) => void;
  setAutoScroll: (autoScroll: boolean) => void;
}

const initialFilter: LogLogFilter = {
  levels: new Set(LOG_LEVELS),
  searchText: "",
};

export const useLogStore = create<LogStore>((set) => ({
  filter: initialFilter,
  selectedLog: null,
  autoScroll: true,

  setSelectedLog: (log) =>
    set((state) => (state.selectedLog === log ? state : { selectedLog: log })),
  toggleLevel: (level) =>
    set((state) => {
      const levels = new Set(state.filter.levels);
      if (levels.has(level)) levels.delete(level);
      else levels.add(level);
      return { filter: { ...state.filter, levels } };
    }),
  setSearchText: (searchText) =>
    set((state) =>
      state.filter.searchText === searchText ? state : { filter: { ...state.filter, searchText } },
    ),
  setAutoScroll: (autoScroll) =>
    set((state) => (state.autoScroll === autoScroll ? state : { autoScroll })),
}));

// List and count consumers share results for immutable buffer/filter inputs. Old inputs do not
// remain alive through the cache when the buffer is trimmed or the filter is replaced.
const filteredEntries = new WeakMap<
  readonly LogRecordDto[],
  WeakMap<LogLogFilter, Partial<Record<LogDomainId, readonly LogRecordDto[]>>>
>();

export function applyLogFilter(
  logs: readonly LogRecordDto[],
  filter: LogLogFilter,
  domain: LogDomainId,
): readonly LogRecordDto[] {
  let filters = filteredEntries.get(logs);
  if (!filters) filteredEntries.set(logs, (filters = new WeakMap()));
  let domains = filters.get(filter);
  if (!domains) filters.set(filter, (domains = {}));
  const cached = domains[domain];
  if (cached) return cached;
  const search = filter.searchText.trim().toLowerCase();
  const result = logs.filter((log) => {
    if (!filter.levels.has(log.level)) return false;
    if (domain !== "all" && log.domain !== domain) return false;
    if (!search) return true;
    return [
      log.message,
      log.source,
      log.domain,
      log.origin,
      log.target,
      log.event,
      JSON.stringify(log.fields),
    ].some((value) => value?.toLowerCase().includes(search));
  });
  domains[domain] = result;
  return result;
}
