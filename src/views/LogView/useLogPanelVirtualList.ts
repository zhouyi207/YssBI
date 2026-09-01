import { useCallback, useLayoutEffect, useRef, type UIEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { LOG_ITEM_GAP, LOG_ITEM_HEIGHT } from "@/shared/config-default";
import type { DiagnosticRecordDto } from "@/shared/types/domain/diagnostics";
import { isLogViewportPinnedToBottom } from "./logPanelScroll";
import { snapLogViewportToBottom } from "./logPanelViewport";

export type LogPanelPresentation = "embedded" | "standalone";

export interface UseLogPanelVirtualListOptions {
  readonly filteredLogs: readonly DiagnosticRecordDto[];
  readonly autoScroll: boolean;
  readonly presentation: LogPanelPresentation;
  readonly refreshScrollToken: number;
}

export function useLogPanelVirtualList({
  filteredLogs,
  autoScroll,
  refreshScrollToken,
}: UseLogPanelVirtualListOptions) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const pinnedToBottomRef = useRef(true);
  const filteredLogsRef = useRef(filteredLogs);
  filteredLogsRef.current = filteredLogs;

  const virtualizer = useVirtualizer({
    count: filteredLogs.length,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => LOG_ITEM_HEIGHT + LOG_ITEM_GAP,
    overscan: 8,
  });

  const snapToBottom = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    snapLogViewportToBottom(viewport, filteredLogsRef.current.length);
    pinnedToBottomRef.current = true;
  }, []);

  useLayoutEffect(() => {
    snapToBottom();
  }, [snapToBottom]);

  useLayoutEffect(() => {
    if (!autoScroll || !pinnedToBottomRef.current) return;
    snapToBottom();
  }, [filteredLogs, autoScroll, snapToBottom]);

  useLayoutEffect(() => {
    if (!autoScroll) {
      pinnedToBottomRef.current = false;
      return;
    }
    snapToBottom();
  }, [autoScroll, snapToBottom]);

  useLayoutEffect(() => {
    if (autoScroll) snapToBottom();
  }, [refreshScrollToken, autoScroll, snapToBottom]);

  const handleScroll = useCallback((event: UIEvent<HTMLDivElement>) => {
    const { scrollTop, scrollHeight, clientHeight } = event.currentTarget;
    pinnedToBottomRef.current = isLogViewportPinnedToBottom(scrollTop, scrollHeight, clientHeight);
  }, []);

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const observer = new ResizeObserver(() => {
      virtualizer.measure();
      if (autoScroll && pinnedToBottomRef.current) snapToBottom();
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [autoScroll, snapToBottom, virtualizer]);

  return { viewportRef, virtualizer, handleScroll };
}
