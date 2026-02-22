import { useCallback, useRef } from 'react';
import { useExecutionStore } from './useExecutionStore';

const DELAYS: Record<string, number> = {
  executionStart: 100,
  nodeStart: 300,
  nodeComplete: 150,
  nodeError: 600,
  connectionActive: 250,
  executionComplete: 100,
};

/**
 * 执行回放引擎
 *
 * 从录制的事件数组中按顺序重放事件，每个事件之间插入可控延迟
 */
export function useExecutionPlayback() {
  const recording = useExecutionStore((s) => s.recording);
  const isPlaying = useExecutionStore((s) => s.isPlaying);
  const hasRecording = recording.length > 0;

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const indexRef = useRef(0);
  const pausedRef = useRef(false);

  const play = useCallback(() => {
    const { resetVisuals, applyEvent, setPlaying } = useExecutionStore.getState();
    const rec = useExecutionStore.getState().recording;
    if (rec.length === 0) return;

    resetVisuals();
    setPlaying(true);
    indexRef.current = 0;
    pausedRef.current = false;

    const step = () => {
      if (pausedRef.current) return;

      const idx = indexRef.current;
      if (idx >= rec.length) {
        setPlaying(false);
        return;
      }

      const entry = rec[idx];
      applyEvent(entry.event);
      indexRef.current = idx + 1;

      const delay = DELAYS[entry.event.event] ?? 200;
      timerRef.current = setTimeout(step, delay);
    };

    step();
  }, []);

  const pause = useCallback(() => {
    pausedRef.current = true;
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const resume = useCallback(() => {
    if (!pausedRef.current) return;
    pausedRef.current = false;
    const { applyEvent, setPlaying } = useExecutionStore.getState();
    const rec = useExecutionStore.getState().recording;

    setPlaying(true);

    const step = () => {
      if (pausedRef.current) return;
      const idx = indexRef.current;
      if (idx >= rec.length) {
        setPlaying(false);
        return;
      }
      const entry = rec[idx];
      applyEvent(entry.event);
      indexRef.current = idx + 1;
      const delay = DELAYS[entry.event.event] ?? 200;
      timerRef.current = setTimeout(step, delay);
    };

    step();
  }, []);

  const stop = useCallback(() => {
    pausedRef.current = false;
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const { resetVisuals } = useExecutionStore.getState();
    resetVisuals();
  }, []);

  const togglePlayPause = useCallback(() => {
    if (pausedRef.current) {
      resume();
    } else if (useExecutionStore.getState().isPlaying) {
      pause();
    } else {
      play();
    }
  }, [play, pause, resume]);

  return {
    play,
    pause,
    resume,
    stop,
    togglePlayPause,
    isPlaying,
    hasRecording,
  };
}
