import { useCallback, useEffect, useRef, useState } from 'react';
import { useExecutionStore } from './useExecutionStore';

const DELAYS: Record<string, number> = {
  executionStart: 100,
  nodeStart: 300,
  nodeComplete: 150,
  nodeError: 600,
  connectionActive: 250,
  executionComplete: 100,
};

export function useExecutionPlayback(graphId: string) {
  const graphState = useExecutionStore((s) => s.graphs[graphId]);
  const recording = graphState?.recording ?? [];
  const isPlaying = useExecutionStore((s) => s.isPlaying && s.playbackGraphId === graphId);
  const hasRecording = recording.length > 0;
  const graphDirty = graphState?.graphDirty ?? false;

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const indexRef = useRef(0);
  const [isPaused, setIsPaused] = useState(false);
  const pausedRef = useRef(false);

  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const scheduleSteps = useCallback((rec: typeof recording) => {
    const { applyEvent, setPlaying } = useExecutionStore.getState();

    const step = () => {
      if (pausedRef.current) return;
      const idx = indexRef.current;
      if (idx >= rec.length) {
        clearTimer();
        setPlaying(false);
        return;
      }
      const entry = rec[idx];
      applyEvent(graphId, entry.event);
      indexRef.current = idx + 1;
      const delay = DELAYS[entry.event.event] ?? 200;
      timerRef.current = setTimeout(step, delay);
    };

    step();
  }, [graphId, clearTimer]);

  const play = useCallback(() => {
    const store = useExecutionStore.getState();
    const rec = store.graphs[graphId]?.recording ?? [];
    if (rec.length === 0) return;

    clearTimer();
    store.resetGraphVisuals(graphId);
    store.setPlaying(true, graphId);
    setIsPaused(false);
    pausedRef.current = false;
    indexRef.current = 0;

    scheduleSteps(rec);
  }, [graphId, scheduleSteps, clearTimer]);

  const pause = useCallback(() => {
    pausedRef.current = true;
    setIsPaused(true);
    clearTimer();
  }, [clearTimer]);

  const resume = useCallback(() => {
    if (!pausedRef.current) return;
    pausedRef.current = false;
    setIsPaused(false);

    const store = useExecutionStore.getState();
    const rec = store.graphs[graphId]?.recording ?? [];
    store.setPlaying(true, graphId);
    scheduleSteps(rec);
  }, [graphId, scheduleSteps]);

  const stop = useCallback(() => {
    pausedRef.current = false;
    setIsPaused(false);
    clearTimer();
    useExecutionStore.getState().resetGraphVisuals(graphId);
  }, [graphId, clearTimer]);

  useEffect(() => {
    return () => {
      clearTimer();
      const store = useExecutionStore.getState();
      if (store.playbackGraphId === graphId) {
        store.setPlaying(false);
      }
    };
  }, [graphId, clearTimer]);

  const togglePlayPause = useCallback(() => {
    if (pausedRef.current) {
      resume();
    } else if (useExecutionStore.getState().isPlaying && useExecutionStore.getState().playbackGraphId === graphId) {
      pause();
    } else {
      play();
    }
  }, [graphId, play, pause, resume]);

  return {
    play,
    pause,
    resume,
    stop,
    togglePlayPause,
    isPlaying,
    isPaused,
    hasRecording,
    graphDirty,
  };
}
