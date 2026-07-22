import { useEffect, useRef, useState } from 'react';
import type { BayesInferenceTaskDTO } from '@/shared/types/bayes';
import { Progress } from '@/components/ui/progress';

export function BayesProgressStatus({ task, stageOverride }: { task: BayesInferenceTaskDTO; stageOverride?: string }) {
  const [now, setNow] = useState(() => Date.now());
  const totalStartedAt = useRef(Date.now());
  const stageStartedAt = useRef(Date.now());
  const lastSample = useRef<{ completed: number; at: number } | null>(null);
  const smoothedRate = useRef<number | null>(null);
  const progress = task.progress;
  const stage = stageOverride ?? progress?.stage ?? task.status;
  const completed = progress?.completed;
  const total = progress?.total;

  useEffect(() => {
    const timestamp = Date.now();
    totalStartedAt.current = timestamp;
    stageStartedAt.current = timestamp;
    lastSample.current = null;
    smoothedRate.current = null;
    setNow(timestamp);
  }, [task.taskId]);

  useEffect(() => {
    stageStartedAt.current = Date.now();
    setNow(Date.now());
  }, [stage]);

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

  const hasSampleCount = completed !== undefined && total !== undefined && total > 0;
  const percentage = bayesOverallProgress(stage, completed, total);
  const remainingSeconds = hasSampleCount && ['warmup', 'sampling'].includes(stage) && smoothedRate.current && completed > 0
    ? Math.max(0, (total - completed) / smoothedRate.current)
    : null;
  const totalElapsedSeconds = Math.max(0, (now - totalStartedAt.current) / 1_000);
  const stageElapsedSeconds = Math.max(0, (now - stageStartedAt.current) / 1_000);

  return (
    <div className="w-64 space-y-1">
      <div className="flex items-center justify-between gap-3 text-xs">
        <span className="truncate text-foreground">{bayesProgressStageLabel(stage)}</span>
        <span className="shrink-0 font-mono text-muted-foreground">{percentage === null ? '' : `${percentage}%`}</span>
      </div>
      {percentage !== null ? (
        <Progress value={percentage} max={100} className="h-1.5" />
      ) : (
        <div className="h-1.5 overflow-hidden rounded-full bg-muted">
          <div className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
        </div>
      )}
      <div className="flex justify-between gap-3 text-[10px] text-muted-foreground">
        <span>
          {hasSampleCount && ['warmup', 'sampling'].includes(stage)
            ? `${completed.toLocaleString()} / ${total.toLocaleString()} · 本阶段 ${formatDuration(stageElapsedSeconds)}`
            : `本阶段 ${formatDuration(stageElapsedSeconds)}`}
        </span>
        <span>
          {remainingSeconds === null
            ? `总计 ${formatDuration(totalElapsedSeconds)}`
            : `剩余 ${formatDuration(remainingSeconds)} · 总计 ${formatDuration(totalElapsedSeconds)}`}
        </span>
      </div>
    </div>
  );
}

export function bayesOverallProgress(stage: string, completed?: number, total?: number): number | null {
  if (['warmup', 'sampling'].includes(stage) && completed !== undefined && total !== undefined && total > 0) {
    return Math.min(90, Math.round((completed / total) * 90));
  }
  const milestones: Record<string, number> = {
    materializing_data: 1,
    loading_model: 2,
    loading_data: 3,
    preparing_response: 4,
    loading_kernels: 5,
    preparing_kernels: 6,
    building_model: 7,
    initializing_nuts: 8,
    summarizing: 92,
    writing_samples: 94,
    posterior_predictive: 96,
    finalizing: 97,
    reading_result: 98,
    writing_artifacts: 98,
    rendering_result: 99,
  };
  return milestones[stage] ?? null;
}

export function bayesProgressStageLabel(stage: string): string {
  const labels: Record<string, string> = {
    queued: '等待运行',
    running: '正在启动',
    materializing_data: '正在准备数据',
    loading_model: '正在启动 Julia 任务',
    loading_data: '正在读取模型与数据',
    preparing_response: '正在准备响应变量',
    loading_kernels: '正在加载生成的计算 Kernel',
    preparing_kernels: '正在准备 Predictor Kernel',
    building_model: '正在构造先验与 Turing 模型',
    initializing_nuts: '正在特化模型并初始化 NUTS',
    warmup: 'NUTS 预热',
    sampling: '后验采样',
    summarizing: '正在计算参数摘要',
    writing_samples: '正在写入后验样本',
    posterior_predictive: '正在计算后验预测',
    finalizing: '正在完成诊断与产物',
    reading_result: '正在读取结果',
    writing_artifacts: '正在计算结果数据',
    rendering_result: '正在计算并渲染结果数据',
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
