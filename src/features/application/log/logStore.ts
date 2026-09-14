import { create } from "zustand";
import { LOG_LEVELS, type LogLevel, type LogRecordDto } from "@/shared/types/domain/log";
import type { LogDomainId } from "@/features/domain/log/logDomains";

export interface LogLogFilter {
  levels: Set<LogLevel>;
  searchText: string;
}

export interface LogStore {
  filter: LogLogFilter;
  selectedLog: LogRecordDto | null;
  autoScroll: boolean;

  setSelectedLog: (log: LogRecordDto | null) => void;
  setFilter: (filter: Partial<LogLogFilter>) => void;
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

  setSelectedLog: (log) => set({ selectedLog: log }),
  setFilter: (filter) =>
    set((state) => ({
      filter: { ...state.filter, ...filter },
    })),
  toggleLevel: (level) =>
    set((state) => {
      const levels = new Set(state.filter.levels);
      if (levels.has(level)) levels.delete(level);
      else levels.add(level);
      return { filter: { ...state.filter, levels } };
    }),
  setSearchText: (searchText) =>
    set((state) => ({
      filter: { ...state.filter, searchText },
    })),
  setAutoScroll: (autoScroll) => set({ autoScroll }),
}));

export function applyLogFilter(
  logs: readonly LogRecordDto[],
  filter: LogLogFilter,
  domain: LogDomainId,
): LogRecordDto[] {
  const search = filter.searchText.trim().toLowerCase();
  return logs.filter((log) => {
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
}
