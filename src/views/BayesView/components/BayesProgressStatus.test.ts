import { describe, expect, it } from 'vitest';
import { bayesOverallProgress, bayesProgressStageLabel, formatDuration } from './BayesProgressStatus';

describe('Bayes progress presentation', () => {
  it('uses stable user-facing labels for backend stages', () => {
    expect(bayesProgressStageLabel('loading_model')).toBe('正在加载并编译模型');
    expect(bayesProgressStageLabel('warmup')).toBe('NUTS 预热');
    expect(bayesProgressStageLabel('sampling')).toBe('后验采样');
    expect(bayesProgressStageLabel('writing_artifacts')).toBe('正在计算结果数据');
    expect(bayesProgressStageLabel('rendering_result')).toBe('正在计算并渲染结果数据');
  });

  it('reserves progress milestones for output parsing and frontend rendering', () => {
    expect(bayesOverallProgress('sampling', 300, 300)).toBe(90);
    expect(bayesOverallProgress('reading_result')).toBe(94);
    expect(bayesOverallProgress('writing_artifacts')).toBe(97);
    expect(bayesOverallProgress('rendering_result')).toBe(99);
  });

  it('formats elapsed and remaining durations without losing hours', () => {
    expect(formatDuration(5)).toBe('00:05');
    expect(formatDuration(125)).toBe('02:05');
    expect(formatDuration(3_725)).toBe('1:02:05');
  });
});
