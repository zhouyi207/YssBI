import { create } from 'zustand';
import { LogMessage, LogFilter, LogLevel, LogType } from '../shared/types/logging';
import { invoke } from '@tauri-apps/api/core';

interface LogStore {
  logs: LogMessage[];
  filter: LogFilter;
  total: number;
  hasMore: boolean;
  loading: boolean;
  
  // Actions
  addLog: (log: LogMessage) => void;
  setLogs: (logs: LogMessage[]) => void;
  appendLogs: (logs: LogMessage[]) => void;
  clearLogs: () => void;
  setFilter: (filter: Partial<LogFilter>) => void;
  toggleLevel: (level: LogLevel) => void;
  toggleType: (type: LogType) => void;
  setSearchText: (text: string) => void;
  getFilteredLogs: () => LogMessage[];
  
  // 懒加载相关
  loadLogs: (offset: number, limit: number) => Promise<void>;
  loadMoreLogs: () => Promise<void>;
  refreshLogs: () => Promise<void>;
}

const initialFilter: LogFilter = {
  levels: new Set<LogLevel>(['trace', 'debug', 'info', 'warn', 'error']),
  types: new Set<LogType>(['application', 'execution', 'system']),
  searchText: '',
};

export const useLogStore = create<LogStore>((set, get) => ({
  logs: [],
  filter: initialFilter,
  total: 0,
  hasMore: false,
  loading: false,

  addLog: (log) => set((state) => ({
    logs: [...state.logs, log],
    total: state.total + 1,
  })),

  setLogs: (logs) => set({ logs }),
  
  appendLogs: (logs) => set((state) => ({
    logs: [...state.logs, ...logs],
  })),

  clearLogs: () => set({ logs: [], total: 0, hasMore: false }),

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
    return logs.filter((log) => {
      // 过滤级别
      if (!filter.levels.has(log.level)) return false;
      
      // 过滤类型
      if (!filter.types.has(log.log_type)) return false;
      
      // 过滤搜索文本
      if (filter.searchText) {
        const searchLower = filter.searchText.toLowerCase();
        const matchMessage = log.message.toLowerCase().includes(searchLower);
        const matchSource = log.source?.toLowerCase().includes(searchLower);
        if (!matchMessage && !matchSource) return false;
      }
      
      return true;
    });
  },
  
  // 加载日志（从文件）
  loadLogs: async (offset: number, limit: number) => {
    set({ loading: true });
    try {
      const response = await invoke<{
        logs: LogMessage[];
        total: number;
        offset: number;
        limit: number;
        has_more: boolean;
      }>('get_logs', { offset, limit });
      
      set({
        logs: response.logs,
        total: response.total,
        hasMore: response.has_more,
        loading: false,
      });
    } catch (error) {
      console.error('Failed to load logs:', error);
      set({ loading: false });
    }
  },
  
  // 加载更多日志
  loadMoreLogs: async () => {
    const { logs, hasMore, loading } = get();
    if (!hasMore || loading) return;
    
    set({ loading: true });
    try {
      // offset 是从末尾开始的偏移量，当前已加载的数量就是 offset
      const offset = logs.length;
      
      const response = await invoke<{
        logs: LogMessage[];
        total: number;
        offset: number;
        limit: number;
        has_more: boolean;
      }>('get_logs', { offset, limit: 50 });
      
      set((state) => ({
        // 将新加载的日志插入到列表开头（因为它们是更早的日志）
        logs: [...response.logs, ...state.logs],
        total: response.total,
        hasMore: response.has_more,
        loading: false,
      }));
    } catch (error) {
      console.error('Failed to load more logs:', error);
      set({ loading: false });
    }
  },
  
  // 刷新日志（重新加载）
  refreshLogs: async () => {
    const { logs } = get();
    const currentCount = logs.length;
    await get().loadLogs(0, Math.max(currentCount, 50));
  },
}));
