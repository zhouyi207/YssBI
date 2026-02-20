import { create } from 'zustand';
import { LogMessage, LogFilter, LogLevel, LogType } from '@/shared/types/ui';
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
  levels: new Set(['trace', 'debug', 'info', 'warn', 'error'] as LogLevel[]),
  types: new Set(['application', 'execution', 'system'] as LogType[]),
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
    if (!Array.isArray(logs) || !filter?.levels || !filter?.types) return [];
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
  // 后端 get_logs 返回 Vec<LogMessage>（数组），非 { logs, total, has_more }
  loadLogs: async (offset: number, limit: number) => {
    set({ loading: true });
    try {
      const response = await invoke<unknown>('get_logs', { offset, limit });
      const logs: LogMessage[] = Array.isArray(response) ? response : (response as any)?.logs ?? [];
      let total = logs.length;
      let hasMore = logs.length >= limit;
      try {
        const count = await invoke<number>('get_log_count');
        if (typeof count === 'number') {
          total = count;
          hasMore = offset + logs.length < total;
        }
      } catch {
        // get_log_count 失败时使用默认值
      }
      set({
        logs,
        total,
        hasMore,
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
      const offset = Array.isArray(logs) ? logs.length : 0;
      const response = await invoke<unknown>('get_logs', { offset, limit: 50 });
      const newLogs: LogMessage[] = Array.isArray(response) ? response : (response as any)?.logs ?? [];
      let total = offset + newLogs.length;
      let hasMoreResult = newLogs.length >= 50;
      try {
        const count = await invoke<number>('get_log_count');
        if (typeof count === 'number') {
          total = count;
          hasMoreResult = offset + newLogs.length < total;
        }
      } catch {
        // 使用默认值
      }
      set((state) => ({
        logs: [...newLogs, ...(Array.isArray(state.logs) ? state.logs : [])],
        total,
        hasMore: hasMoreResult,
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
