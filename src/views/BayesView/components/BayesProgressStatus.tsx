import { useEffect, useRef, useState } from 'react';
import type { BayesInferenceTaskDTO } from '@/shared/types/bayes';
import { Progress } from '@/components/ui/progress';

export function BayesProgressStatus({ task }: { task: BayesInferenceTaskDTO }) {
  const [now, setNow] = useState(() => Date.now());
  const startedAt = useRef(Date.now());
  const lastSample = useRef<{ completed: number; at: number } | null>(null);
  const smoothedRate = useRef<number | null>(null);
  const progress = task.progress;
  const completed = progress?.completed;
  const total = progress?.total;

  useEffect(() => {
    const timestamp = Date.now();
    startedAt.current = timestamp;
    lastSample.current = null;
    smoothedRate.current = null;
    setNow(timestamp);
  }, [task.taskId]);

  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [task.taskId]);

  useEffect(() => {
    if (completed === undefined) return;
    const timestamp = Date.now();
    const previous = lastSample.current;
    if (previous && completed > previous.completed && timestamp > previous.at) {
      const currentRate = (completed - previous.completed) / ((timestamp - previous.at) / 1_000);
      smoothedRate.current = smoothedRate.current === null
        ? currentRate
        : smoothedRate.current * 0.8 + currentRate * 0.2;
    }
    lastSample.current = { completed, at: timestamp };
  }, [completed]);

  const hasCount = completed !== undefined && total !== undefined && total > 0;
  const percentage = hasCount ? Math.min(100, Math.round((completed / total) * 100)) : null;
  const remainingSeconds = hasCount && smoothedRate.current && completed > 0
    ? Math.max(0, (total - completed) / smoothedRate.current)
    : null;
  const elapsedSeconds = Math.max(0, (now - startedAt.current) / 1_000);

  return (
    <div className="w-64 space-y-1">
      <div className="flex items-center justify-between gap-3 text-xs">
        <span className="truncate text-foreground">{bayesProgressStageLabel(progress?.stage ?? task.status)}</span>
        <span className="shrink-0 font-mono text-muted-foreground">{percentage === null ? '' : `${percentage}%`}</span>
      </div>
      {hasCount ? (
        <Progress value={completed} max={total} className="h-1.5" />
      ) : (
        <div className="h-1.5 overflow-hidden rounded-full bg-muted">
          <div className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
        </div>
      )}
      <div className="flex justify-between gap-3 text-[10px] text-muted-foreground">
        <span>
          {hasCount
            ? `${completed.toLocaleString()} / ${total.toLocaleString()} · ${formatDuration(elapsedSeconds)}`
            : `已运行 ${formatDuration(elapsedSeconds)}`}
        </span>
        <span>{remainingSeconds === null ? (hasCount ? '正在估算' : '') : `预计剩余 ${formatDuration(remainingSeconds)}`}</span>
      </div>
    </div>
  );
}

export function bayesProgressStageLabel(stage: string): string {
  const labels: Record<string, string> = {
    queued: '等待运行',
    running: '正在启动',
    materializing_data: '正在准备数据',
    loading_model: '正在加载并编译模型',
    warmup: 'NUTS 预热',
    sampling: '后验采样',
    reading_result: '正在读取结果',
    writing_artifacts: '正在保存结果',
    cancelling: '正在取消',
  };
  return labels[stage] ?? stage;
}

export function formatDuration(seconds: number): string {
  const rounded = Math.max(0, Math.round(seconds));
  const hours = Math.floor(rounded / 3_600);
  const minutes = Math.floor((rounded % 3_600) / 60);
  const remaining = rounded % 60;
  if (hours > 0) return `${hours}:${String(minutes).padStart(2, '0')}:${String(remaining).padStart(2, '0')}`;
  return `${String(minutes).padStart(2, '0')}:${String(remaining).padStart(2, '0')}`;
}
