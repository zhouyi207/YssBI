import { create } from 'zustand';
import {
  DIAGNOSTIC_LEVELS,
  type DiagnosticDomain,
  type DiagnosticLevel,
  type DiagnosticRecordDto,
} from '@/shared/types/dto/diagnostics';
import { domainsForLogDomainTab, type LogDomainTabId } from './logDomainTabs';

export interface DiagnosticLogFilter {
  levels: Set<DiagnosticLevel>;
  domains: Set<DiagnosticDomain> | null;
  searchText: string;
}

export interface LogStore {
  filter: DiagnosticLogFilter;
  activeLogDomainTab: LogDomainTabId;
  selectedLog: DiagnosticRecordDto | null;

  setSelectedLog: (log: DiagnosticRecordDto | null) => void;
  setFilter: (filter: Partial<DiagnosticLogFilter>) => void;
  setActiveLogDomainTab: (tab: LogDomainTabId) => void;
  toggleLevel: (level: DiagnosticLevel) => void;
  setSearchText: (text: string) => void;
}

const initialFilter: DiagnosticLogFilter = {
  levels: new Set(DIAGNOSTIC_LEVELS),
  domains: null,
  searchText: '',
};

export const useLogStore = create<LogStore>((set) => ({
  filter: initialFilter,
  activeLogDomainTab: 'all',
  selectedLog: null,

  setSelectedLog: (log) => set({ selectedLog: log }),
  setFilter: (filter) => set((state) => ({
    filter: { ...state.filter, ...filter },
  })),
  setActiveLogDomainTab: (tab) => set((state) => ({
    activeLogDomainTab: tab,
    filter: { ...state.filter, domains: domainsForLogDomainTab(tab) },
  })),
  toggleLevel: (level) => set((state) => {
    const levels = new Set(state.filter.levels);
    if (levels.has(level)) levels.delete(level);
    else levels.add(level);
    return { filter: { ...state.filter, levels } };
  }),
  setSearchText: (searchText) => set((state) => ({
    filter: { ...state.filter, searchText },
  })),
}));

export function applyLogFilter(
  logs: readonly DiagnosticRecordDto[],
  filter: DiagnosticLogFilter,
): DiagnosticRecordDto[] {
  const search = filter.searchText.trim().toLowerCase();
  return logs.filter((log) => {
    if (!filter.levels.has(log.level)) return false;
    if (filter.domains && !filter.domains.has(log.domain)) return false;
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
