// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ResultDescriptor } from '@/shared/types/dto/result';
import { ReportView } from './ReportView';

const { logError } = vi.hoisted(() => ({ logError: vi.fn() }));

vi.mock('@/utils/appLogger', () => ({
  logger: { notify: { error: logError } },
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const descriptor: ResultDescriptor = {
  resultId: '42',
  state: { kind: 'ready' },
  provenance: {
    runId: '7',
    activationId: '9',
    graphPath: 'events/main.yssbi-event',
    graphRevision: '3',
    nodeId: 'ols-node',
    output: {
      graphPath: 'events/main.yssbi-event',
      port: { kind: 'declared', nodeId: 'ols-node', portKey: 'report' },
    },
    createdAtMs: '100',
  },
  presentation: { kind: 'report', report: 'olsSummary' },
  valueKind: 'scalar',
  metadata: null,
  totalCount: 1,
  title: 'OLS Summary',
};

const malformedOlsReport = {
  title: 'OLS Summary',
  model_basic_info: {
    model_type: 'OLS',
    method: 'Least Squares',
    num_observation: 3,
    r_squared: 0.8,
    adj_r_squared: 0.7,
    f_statistic: 8,
    prob_f_statistic: 0.05,
    df_model: 1,
    df_residual: 1,
    df_total: 2,
    ss_model: 2,
    ss_residual: 0.5,
    ss_total: 2.5,
    ms_model: 2,
    ms_residual: 0.5,
    ms_total: 1.25,
    covariance_type: 'nonrobust',
  },
  coefficients: [{
    variable: 'x',
    coef: 1,
    t_value: 2,
    p_value: 0.05,
    'confidence_interval_0.025': 0.5,
    'confidence_interval_0.975': 1.5,
    is_significant: true,
  }],
  diagnostic_info: { cond_no: 1 },
};

describe('ReportView', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    logError.mockClear();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('logs an actionable diagnostic for a malformed canonical OLS report', () => {
    act(() => {
      root.render(
        <ReportView
          descriptor={descriptor}
          report="olsSummary"
          data={malformedOlsReport}
        />,
      );
    });

    expect(container.textContent).toBe(
      'Unable to render OLS report: coefficients[0].std_err missing required field.',
    );
    expect(logError).toHaveBeenCalledTimes(1);
    expect(JSON.parse(logError.mock.calls[0][0])).toEqual({
      resultId: '42',
      runId: '7',
      activationId: '9',
      nodeId: 'ols-node',
      outputPinId: 'report',
      presentation: { kind: 'report', report: 'olsSummary' },
      valueKind: 'scalar',
      fieldPath: 'coefficients[0].std_err',
      reason: 'missing required field',
    });
    expect(logError).toHaveBeenCalledWith(expect.any(String), 'ReportValidation');
  });

  it('reports the exact missing OLS model field path', () => {
    act(() => {
      root.render(
        <ReportView
          descriptor={descriptor}
          report="olsSummary"
          data={{
            ...malformedOlsReport,
            model_basic_info: {
              ...malformedOlsReport.model_basic_info,
              covariance_type: undefined,
            },
            coefficients: [{ ...malformedOlsReport.coefficients[0], std_err: 0.2 }],
          }}
        />,
      );
    });

    expect(container.textContent).toBe(
      'Unable to render OLS report: model_basic_info.covariance_type missing required field.',
    );
    expect(logError).toHaveBeenCalledTimes(1);
    expect(JSON.parse(logError.mock.calls[0][0])).toMatchObject({
      resultId: '42',
      runId: '7',
      activationId: '9',
      nodeId: 'ols-node',
      fieldPath: 'model_basic_info.covariance_type',
      reason: 'missing required field',
    });
  });
});
