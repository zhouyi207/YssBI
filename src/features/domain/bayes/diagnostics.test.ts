import { describe, expect, it } from 'vitest';
import type { InferenceResultDTO, ParameterSummaryDTO } from '@/shared/types/bayes';
import {
  describeDiagnosticWarning,
  evaluateInferenceDiagnostics,
  parameterDiagnosticStatus,
} from './diagnostics';

function summary(overrides: Partial<ParameterSummaryDTO> = {}): ParameterSummaryDTO {
  return {
    parameter: 'a',
    mean: 1,
    sd: 0.1,
    median: 1,
    q025: 0.8,
    q975: 1.2,
    rhat: 1.0,
    essBulk: 500,
    essTail: 400,
    ...overrides,
  };
}

function result(summaries: ParameterSummaryDTO[], diagnostics: Partial<InferenceResultDTO['diagnostics']> = {}): InferenceResultDTO {
  return {
    summaries,
    artifactManifest: { taskId: 'task-1', artifacts: [] },
    diagnostics: {
      chains: 2,
      drawsPerChain: 1000,
      warmup: 500,
      divergences: 0,
      maxTreedepthHits: 0,
      warnings: [],
      ...diagnostics,
    },
  };
}

describe('Bayesian diagnostics assessment', () => {
  it('marks healthy summaries as good', () => {
    expect(evaluateInferenceDiagnostics(result([summary()])).severity).toBe('good');
    expect(parameterDiagnosticStatus(summary())).toBe('ok');
  });

  it('marks high rhat as bad or check_rhat', () => {
    expect(evaluateInferenceDiagnostics(result([summary({ rhat: 1.2 })])).severity).toBe('bad');
    expect(parameterDiagnosticStatus(summary({ rhat: 1.02 }))).toBe('check_rhat');
  });

  it('marks low ESS as warning', () => {
    expect(evaluateInferenceDiagnostics(result([summary({ essBulk: 50 })])).severity).toBe('warning');
    expect(parameterDiagnosticStatus(summary({ essTail: 50 }))).toBe('low_ess');
  });

  it('marks divergences as bad', () => {
    const assessment = evaluateInferenceDiagnostics(result([summary()], { divergences: 1 }));
    expect(assessment.severity).toBe('bad');
    expect(assessment.metrics.find(metric => metric.key === 'divergences')?.severity).toBe('bad');
  });

  it('marks treedepth hits as warning', () => {
    expect(evaluateInferenceDiagnostics(result([summary()], { maxTreedepthHits: 1 })).severity).toBe('warning');
  });

  it('reports unavailable treedepth diagnostics as unknown instead of zero', () => {
    const assessment = evaluateInferenceDiagnostics(result([summary()], { maxTreedepthHits: null }));
    expect(assessment.metrics.find(metric => metric.key === 'max_treedepth_hits')).toMatchObject({
      label: 'Max treedepth hits: unavailable',
      severity: 'unknown',
    });
  });

  it('treats and describes every backend warning in the domain assessment', () => {
    const warnings = [{ code: 'BACKEND_SPECIFIC', message: 'check this result' }];
    const assessment = evaluateInferenceDiagnostics(result([summary()], { warnings }));
    expect(assessment.severity).toBe('warning');
    expect(assessment.warnings).toMatchObject([{ code: 'BACKEND_SPECIFIC', explanation: 'check this result' }]);
  });

  it('describes stable backend warning codes', () => {
    const description = describeDiagnosticWarning({ code: 'RHAT_TOO_HIGH', message: 'raw', parameter: 'a' });
    expect(description.title).toContain('a');
    expect(description.suggestion).toContain('warmup');
  });
});
