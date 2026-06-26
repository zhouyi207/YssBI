/**
 * useLiveLogs - 通过 useSyncExternalStore 订阅 logBuffer
 *
 * React 18+ 的外部存储桥接：logBuffer 在一帧内合并多次写入，这里只在每帧
 * 拿到一个稳定快照，从而把「高频日志流」转化为「每帧最多一次重渲染」。
 */
import { useSyncExternalStore } from 'react';
import { logBuffer, type LogSnapshot } from './logBuffer';

export function useLiveLogs(): LogSnapshot {
  return useSyncExternalStore(logBuffer.subscribe, logBuffer.getSnapshot);
}
