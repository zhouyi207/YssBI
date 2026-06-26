import { create } from 'zustand';
import { LogMessage, LogFilter, LogLevel, LogType } from '@/shared/types/ui';

/**
 * logStore - 日志「冷」控制状态
 *
 * 仅保存随人类操作（点击筛选 / 选中行）变化的低频状态。高频的日志列表数据
 * 不在这里——它放在 React 之外的 `logBuffer`（见 logBuffer.ts / useLiveLogs.ts），
 * 避免每条日志触发一次 store 提交与全量重渲染。
 */
export interface LogStore {
  filter: LogFilter;
  selectedLog: LogMessage | null;

  setSelectedLog: (log: LogMessage | null) => void;
  setFilter: (filter: Partial<LogFilter>) => void;
  toggleLevel: (level: LogLevel) => void;
  toggleType: (type: LogType) => void;
  setSearchText: (text: string) => void;
}

const initialFilter: LogFilter = {
  levels: new Set(['trace', 'debug', 'info', 'warn', 'error'] as LogLevel[]),
  types: new Set(['application', 'execution', 'system', 'graph', 'data'] as LogType[]),
  searchText: '',
};

export const useLogStore = create<LogStore>((set) => ({
  filter: initialFilter,
  selectedLog: null,

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
}));

/**
 * 纯函数：按筛选条件过滤日志。供 LogPanelContent 以 `useMemo([entries, filter])` 调用，
 * 取代此前在 render body 内每次重算的 `getFilteredLogs`。
 */
export function applyLogFilter(logs: LogMessage[], filter: LogFilter): LogMessage[] {
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
}
