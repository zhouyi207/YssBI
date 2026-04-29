import { create } from 'zustand';
import { LogMessage, LogFilter, LogLevel, LogType } from '@/shared/types/ui';

export interface LogStore {
  logs: LogMessage[];
  filter: LogFilter;
  total: number;
  hasMore: boolean;
  loading: boolean;
  selectedLog: LogMessage | null;
  
  addLog: (log: LogMessage) => void;
  setLogs: (logs: LogMessage[]) => void;
  appendLogs: (logs: LogMessage[]) => void;
  prependLogs: (logs: LogMessage[]) => void;
  setLogPageState: (state: { total?: number; hasMore?: boolean; loading?: boolean }) => void;
  clearLogs: () => void;
  setSelectedLog: (log: LogMessage | null) => void;
  setFilter: (filter: Partial<LogFilter>) => void;
  toggleLevel: (level: LogLevel) => void;
  toggleType: (type: LogType) => void;
  setSearchText: (text: string) => void;
  getFilteredLogs: () => LogMessage[];
}

const initialFilter: LogFilter = {
  levels: new Set(['trace', 'debug', 'info', 'warn', 'error'] as LogLevel[]),
  types: new Set(['application', 'execution', 'system', 'graph', 'data'] as LogType[]),
  searchText: '',
};

export const useLogStore = create<LogStore>((set, get) => ({
  logs: [],
  filter: initialFilter,
  total: 0,
  hasMore: false,
  loading: false,
  selectedLog: null,

  addLog: (log) => set((state) => ({
    logs: [...state.logs, log],
    total: state.total + 1,
  })),

  setLogs: (logs) => set({ logs }),
  
  appendLogs: (logs) => set((state) => ({
    logs: [...state.logs, ...logs],
  })),

  prependLogs: (logs) => set((state) => ({
    logs: [...logs, ...(Array.isArray(state.logs) ? state.logs : [])],
  })),

  setLogPageState: (pageState) => set(pageState),

  clearLogs: () => set({ logs: [], total: 0, hasMore: false, selectedLog: null }),

  setSelectedLog: (log) => set({ selectedLog: log }),

  setFilter: (newFilter) => set((state) => ({
    filter: { ...state.filter, ...newFilter },
  })),

  toggleLevel: (level) => set((state) => {
    const newLevels = new Set(state.filter.levels);
    if (newLevels.has(level)) {
      newLevels.delete(level);
    } else {
      newLevels.add(level);
    }
    return {
      filter: { ...state.filter, levels: newLevels },
    };
  }),

  toggleType: (type) => set((state) => {
    const newTypes = new Set(state.filter.types);
    if (newTypes.has(type)) {
      newTypes.delete(type);
    } else {
      newTypes.add(type);
    }
    return {
      filter: { ...state.filter, types: newTypes },
    };
  }),

  setSearchText: (text) => set((state) => ({
    filter: { ...state.filter, searchText: text },
  })),

  getFilteredLogs: () => {
    const { logs, filter } = get();
    if (!Array.isArray(logs) || !filter?.levels || !filter?.types) return [];
    return logs.filter((log) => {
      if (!filter.levels.has(log.level)) return false;
      if (!filter.types.has(log.log_type)) return false;
      if (filter.searchText) {
        const searchLower = filter.searchText.toLowerCase();
        const matchMessage = log.message.toLowerCase().includes(searchLower);
        const matchSource = log.source?.toLowerCase().includes(searchLower);
        if (!matchMessage && !matchSource) return false;
      }
      return true;
    });
  },
}));
