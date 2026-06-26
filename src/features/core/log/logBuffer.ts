/**
 * logBuffer - React 之外的实时日志缓冲（外部存储）
 *
 * 目的：日志是高频可观测数据流，若每条 `log-message` 都驱动一次 React 状态更新，
 * 就会「每来一条 = 一次组件提交」，与画布抢占同一条主线程造成卡顿。
 *
 * 设计（对标 VSCode OutputChannel：节流写入 + 视口虚拟化）：
 *   - 数据放在模块级、容量有界的环形缓冲里，`pushLive` 为 O(1)，完全不碰 React；
 *   - 通过 rAF 合并通知：一帧内涌入多少条，都只产生一次快照、通知一次订阅者；
 *   - 经 `useSyncExternalStore` 暴露给 React（`getSnapshot` 引用稳定，仅在 flush 时变化）。
 *
 * 历史分页（向上滚动加载更旧日志）仍以后端为事实来源：`setInitial` / `prependOlder`
 * 写入缓冲，并用 `backendCount` 跟踪「已从后端加载的条数」作为下一次分页 offset，
 * 避免实时追加污染 offset。
 */
import { LOG_BUFFER_MAX } from '@/app/appConfig/default';
import type { LogMessage } from '@/shared/types/ui';

export interface LogSnapshot {
  entries: LogMessage[];
  total: number;
  hasMore: boolean;
  loading: boolean;
}

const EMPTY_SNAPSHOT: LogSnapshot = { entries: [], total: 0, hasMore: false, loading: false };

let backing: LogMessage[] = [];
let total = 0;
let hasMore = false;
let loading = false;
/** 已从后端历史加载的条数，作为下一次分页 offset（不含实时追加） */
let backendCount = 0;

let snapshot: LogSnapshot = EMPTY_SNAPSHOT;
let dirty = false;
let frame: number | null = null;
const listeners = new Set<() => void>();

function rebuildSnapshot(): void {
  snapshot = { entries: backing.slice(), total, hasMore, loading };
  dirty = false;
}

function notify(): void {
  for (const cb of listeners) cb();
}

/** 立即提交（用于人类操作频率的写入：初始加载 / 分页 / 清空 / loading 切换） */
function commitNow(): void {
  if (frame !== null) {
    cancelAnimationFrame(frame);
    frame = null;
  }
  rebuildSnapshot();
  notify();
}

/** 帧级合并提交（用于高频实时流） */
function scheduleFlush(): void {
  dirty = true;
  if (frame !== null) return;
  if (listeners.size === 0) return; // 无订阅者时不调度，仅标记 dirty
  frame = requestAnimationFrame(() => {
    frame = null;
    if (!dirty) return;
    rebuildSnapshot();
    notify();
  });
}

// ─── useSyncExternalStore 接口 ───

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  // 若在无订阅期间累积过实时日志，挂载时立即刷新一次，让新读者看到最新数据
  if (dirty) commitNow();
  return () => {
    listeners.delete(cb);
  };
}

function getSnapshot(): LogSnapshot {
  return snapshot;
}

// ─── 写入接口 ───

/** 实时流：单条追加，O(1)，超出上限丢弃最旧（帧级合并通知） */
function pushLive(log: LogMessage): void {
  backing.push(log);
  if (backing.length > LOG_BUFFER_MAX) {
    backing.splice(0, backing.length - LOG_BUFFER_MAX);
  }
  total += 1;
  scheduleFlush();
}

/** 初始加载 / 刷新：整体替换 */
function setInitial(logs: LogMessage[], nextTotal: number, nextHasMore: boolean): void {
  backing = logs.length > LOG_BUFFER_MAX ? logs.slice(logs.length - LOG_BUFFER_MAX) : [...logs];
  backendCount = backing.length;
  total = nextTotal;
  hasMore = nextHasMore;
  loading = false;
  commitNow();
}

/** 历史分页：把更旧的日志前插 */
function prependOlder(olderLogs: LogMessage[], nextTotal: number, nextHasMore: boolean): void {
  if (olderLogs.length > 0) {
    backing = [...olderLogs, ...backing];
  }
  backendCount += olderLogs.length;
  total = nextTotal;
  hasMore = nextHasMore;
  loading = false;
  commitNow();
}

function setLoading(value: boolean): void {
  if (loading === value) return;
  loading = value;
  commitNow();
}

function clear(): void {
  backing = [];
  total = 0;
  hasMore = false;
  backendCount = 0;
  loading = false;
  commitNow();
}

/** 下一次后端分页的 offset（已加载的历史条数，不含实时追加） */
function getBackendCount(): number {
  return backendCount;
}

export const logBuffer = {
  subscribe,
  getSnapshot,
  pushLive,
  setInitial,
  prependOlder,
  setLoading,
  clear,
  getBackendCount,
};
