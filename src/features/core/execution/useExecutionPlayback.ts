import { useCallback, useEffect, useRef, useState } from 'react';
import { useExecutionStore } from './useExecutionStore';
import { ensureGraphExecutionTerminal, recordingHadError } from './executionRecording';
import { applyExecutionVisualEvent, resetExecutionVisual } from './executionVisualSession';
import {
  EXECUTION_REPLAY_DEFAULT_DELAY_MS,
  EXECUTION_REPLAY_DELAYS_MS,
} from './executionReplayDelays';

export function useExecutionPlayback(graphPath: string) {
  const graphState = useExecutionStore((s) => s.graphs[graphPath]);
  const recording = graphState?.recording ?? [];
  const isPlaying = useExecutionStore((s) => s.isPlaying && s.playbackGraphPath === graphPath);
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
    const { setPlaying, applySideEffectEvent, commitExecutionVisual } =
      useExecutionStore.getState();

    const step = () => {
      if (pausedRef.current) return;
      const idx = indexRef.current;
      if (idx >= rec.length) {
        clearTimer();
        commitExecutionVisual(graphPath);
        ensureGraphExecutionTerminal(
          graphPath,
          recordingHadError(rec) ? 'error' : 'success',
        );
        setPlaying(false);
        return;
      }
      const entry = rec[idx];
      const event = entry.event;
      if (event.event === 'pinResultReady') {
        applySideEffectEvent(graphPath, event);
      } else if (event.event === 'executionStart') {
        // Do not call startExecution — it clears recording and breaks repeat replay.
        resetExecutionVisual(graphPath);
      } else {
        applyExecutionVisualEvent(graphPath, event);
      }
      indexRef.current = idx + 1;
      const delay = EXECUTION_REPLAY_DELAYS_MS[event.event] ?? EXECUTION_REPLAY_DEFAULT_DELAY_MS;
      timerRef.current = setTimeout(step, delay);
    };

    step();
  }, [graphPath, clearTimer]);

  const play = useCallback(() => {
    const store = useExecutionStore.getState();
    const rec = [...(store.graphs[graphPath]?.recording ?? [])];
    if (rec.length === 0) return;

    clearTimer();
    store.resetGraphVisuals(graphPath);
    store.setPlaying(true, graphPath);
    setIsPaused(false);
    pausedRef.current = false;
    indexRef.current = 0;

    scheduleSteps(rec);
  }, [graphPath, scheduleSteps, clearTimer]);

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
    store.setPlaying(true, graphPath);
    scheduleSteps(store.graphs[graphPath]?.recording ?? []);
  }, [graphPath, scheduleSteps]);

  const stop = useCallback(() => {
    pausedRef.current = false;
    setIsPaused(false);
    clearTimer();
    useExecutionStore.getState().resetGraphVisuals(graphPath);
  }, [graphPath, clearTimer]);

  useEffect(() => {
    return () => {
      clearTimer();
      const store = useExecutionStore.getState();
      if (store.playbackGraphPath === graphPath) {
        store.setPlaying(false);
      }
    };
  }, [graphPath, clearTimer]);

  const togglePlayPause = useCallback(() => {
    if (pausedRef.current) {
      resume();
    } else if (useExecutionStore.getState().isPlaying && useExecutionStore.getState().playbackGraphPath === graphPath) {
      pause();
    } else {
      play();
    }
  }, [graphPath, play, pause, resume]);

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
