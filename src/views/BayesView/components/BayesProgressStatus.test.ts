import { describe, expect, it } from 'vitest';
import { bayesProgressStageLabel, formatDuration } from './BayesProgressStatus';

describe('Bayes progress presentation', () => {
  it('uses stable user-facing labels for backend stages', () => {
    expect(bayesProgressStageLabel('loading_model')).toBe('正在加载并编译模型');
    expect(bayesProgressStageLabel('warmup')).toBe('NUTS 预热');
    expect(bayesProgressStageLabel('sampling')).toBe('后验采样');
  });

  it('formats elapsed and remaining durations without losing hours', () => {
    expect(formatDuration(5)).toBe('00:05');
    expect(formatDuration(125)).toBe('02:05');
    expect(formatDuration(3_725)).toBe('1:02:05');
  });
});
