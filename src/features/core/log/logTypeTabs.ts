import { LogType } from '@/shared/types/ui';

export type LogTypeTabId = LogType | 'all';

export const ALL_LOG_TYPES: LogType[] = [
  LogType.Application,
  LogType.Execution,
  LogType.System,
  LogType.Graph,
  LogType.Data,
  LogType.Notify,
];

export const LOG_TYPE_TAB_ORDER: LogTypeTabId[] = ['all', ...ALL_LOG_TYPES];

export function typesForLogTypeTab(tab: LogTypeTabId): Set<LogType> {
  if (tab === 'all') return new Set(ALL_LOG_TYPES);
  return new Set([tab]);
}
