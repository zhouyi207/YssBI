import { describe, expect, it } from 'vitest';
import { bayesOverallProgress, bayesProgressStageLabel, formatDuration } from './BayesProgressStatus';

describe('Bayes progress presentation', () => {
  it('uses stable user-facing labels for backend stages', () => {
    expect(bayesProgressStageLabel('loading_model')).toBe('正在启动 Julia 任务');
    expect(bayesProgressStageLabel('loading_data')).toBe('正在读取模型与数据');
    expect(bayesProgressStageLabel('loading_kernels')).toBe('正在加载生成的计算 Kernel');
    expect(bayesProgressStageLabel('building_model')).toBe('正在构造先验与 Turing 模型');
    expect(bayesProgressStageLabel('initializing_nuts')).toBe('正在特化模型并初始化 NUTS');
    expect(bayesProgressStageLabel('warmup')).toBe('NUTS 预热');
    expect(bayesProgressStageLabel('sampling')).toBe('后验采样');
    expect(bayesProgressStageLabel('summarizing')).toBe('正在计算参数摘要');
    expect(bayesProgressStageLabel('posterior_predictive')).toBe('正在计算后验预测');
    expect(bayesProgressStageLabel('writing_artifacts')).toBe('正在计算结果数据');
    expect(bayesProgressStageLabel('rendering_result')).toBe('正在计算并渲染结果数据');
  });

  it('reserves progress milestones for output parsing and frontend rendering', () => {
    expect(bayesOverallProgress('initializing_nuts')).toBe(8);
    expect(bayesOverallProgress('sampling', 300, 300)).toBe(90);
    expect(bayesOverallProgress('summarizing')).toBe(92);
    expect(bayesOverallProgress('posterior_predictive')).toBe(96);
    expect(bayesOverallProgress('reading_result')).toBe(98);
    expect(bayesOverallProgress('writing_artifacts')).toBe(98);
    expect(bayesOverallProgress('rendering_result')).toBe(99);
  });

  it('formats elapsed and remaining durations without losing hours', () => {
    expect(formatDuration(5)).toBe('00:05');
    expect(formatDuration(125)).toBe('02:05');
    expect(formatDuration(3_725)).toBe('1:02:05');
  });
});
