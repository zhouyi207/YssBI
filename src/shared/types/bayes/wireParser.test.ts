import { describe, expect, it } from 'vitest';
import {
  parseBayesInferenceTaskDTO,
  parseInferenceResultDTO,
  parseValidationReportDTO,
} from './wireParser';

const validTask = {
  taskId: 'task-42',
  status: 'failed',
  progress: null,
  error: {
    code: 'julia_bayes_invalid_data',
    details: {
      column: 'predictor_x',
      row: 7,
      parameter: 'beta',
      path: 'parameters.beta',
    },
    incidentId: null,
  },
} as const;

const validResult = {
  summaries: [],
  diagnostics: {
    chains: 4,
    drawsPerChain: 1_000,
    warmup: 500,
    divergences: 0,
    maxTreedepthHits: null,
    warnings: [{
      code: 'ess_too_low',
      metric: 'ess_tail',
      value: 42.5,
      threshold: 100,
      parameter: 'beta',
    }],
  },
  artifactManifest: {
    taskId: 'task-42',
    artifacts: [],
  },
} as const;

const validValidation = {
  ok: false,
  errors: [{
    code: 'parameter_prior_args_invalid',
    severity: 'error',
    path: 'parameters.beta.prior',
  }],
  warnings: [],
} as const;

describe('Bayes wire parsers', () => {
  it('accepts safe task details and a null incident ID', () => {
    expect(parseBayesInferenceTaskDTO(validTask)).toEqual(validTask);
  });

  it.each(['message', 'detail', 'hint'])('rejects legacy TaskError field %s', (field) => {
    expect(() => parseBayesInferenceTaskDTO({
      ...validTask,
      error: { ...validTask.error, [field]: 'private backend prose' },
    })).toThrow('Invalid Bayes inference task response');
  });

  it('rejects prose and unknown keys nested in TaskError details', () => {
    expect(() => parseBayesInferenceTaskDTO({
      ...validTask,
      error: {
        ...validTask.error,
        details: { column: 'x', message: 'private backend prose' },
      },
    })).toThrow('Invalid Bayes inference task response');
  });

  it('accepts structured diagnostic warnings and rejects legacy prose', () => {
    expect(parseInferenceResultDTO(validResult)).toEqual(validResult);
    expect(() => parseInferenceResultDTO({
      ...validResult,
      diagnostics: {
        ...validResult.diagnostics,
        warnings: [{ ...validResult.diagnostics.warnings[0], hint: 'increase samples' }],
      },
    })).toThrow('Invalid Bayes inference result response');
  });

  it('accepts machine-readable validation issues and rejects legacy prose', () => {
    expect(parseValidationReportDTO(validValidation)).toEqual(validValidation);
    expect(() => parseValidationReportDTO({
      ...validValidation,
      errors: [{ ...validValidation.errors[0], message: 'invalid prior' }],
    })).toThrow('Invalid Bayes validation response');
  });
});
