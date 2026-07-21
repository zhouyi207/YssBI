import { describe, expect, it } from 'vitest';
import {
  normalizeDurbinWatsonResult,
  normalizeSerialTestsResponse,
} from './serialTests';
import {
  acfSeriesToBars,
  hasLjungBoxStats,
  parsePlotCorrelogramBar,
  pacfSeriesToBars,
} from './correlogram';
import { parseIv2slsFirstStageResult } from './iv';
import { parseRegressionResultData } from './parseRegression';
import { parseReportPayload } from './parseReportPayload';

describe('normalizeSerialTestsResponse', () => {
  it('accepts dw as { d: number }', () => {
    const result = normalizeSerialTestsResponse({
      dw: { d: 1.85 },
      q: { stat: 2.1, p_value: 0.3, lags: 5 },
    });
    expect(result?.dw.d).toBe(1.85);
    expect(result?.q?.lags).toBe(5);
  });

  it('rejects bare number dw (historical bug shape)', () => {
    expect(normalizeSerialTestsResponse({ dw: 1.85 })).toBeNull();
    expect(normalizeDurbinWatsonResult(1.85)).toBeNull();
  });

  it('rejects malformed bg while keeping dw', () => {
    expect(
      normalizeSerialTestsResponse({
        dw: { d: 2 },
        bg: { stat: 1, p_value: 0.5 },
      }),
    ).toBeNull();
  });
});

describe('correlogram report DTO', () => {
  it('builds report bars without ljung-box stats', () => {
    const acf = acfSeriesToBars([1, 0.5, 0.2]);
    expect(acf[0]).toEqual({ lag: 0, value: 1 });
    expect(hasLjungBoxStats(acf[0])).toBe(false);
    const pacf = pacfSeriesToBars([0.5, 0.1]);
    expect(pacf[0].lag).toBe(1);
  });

  it('parses plot bar with required q_stat and p_value', () => {
    const bar = parsePlotCorrelogramBar({
      lag: 2,
      value: 0.3,
      q_stat: 1.2,
      p_value: 0.04,
    });
    expect(bar).not.toBeNull();
    expect(hasLjungBoxStats(bar!)).toBe(true);
    expect(parsePlotCorrelogramBar({ lag: 1, value: 0.2 })).toBeNull();
  });
});

describe('parseIv2slsFirstStageResult', () => {
  it('accepts Rust-shaped first stage JSON', () => {
    const result = parseIv2slsFirstStageResult({
      endog_name: 'y1',
      var_names: ['x1', 'z1'],
      r_squared: 0.42,
      adj_r_squared: 0.4,
      coefficients: [
        {
          variable: 'x1',
          coef: 1.2,
          std_err: 0.3,
          t_value: 4,
          p_value: 0.001,
          is_significant: true,
        },
      ],
    });
    expect(result?.endog_name).toBe('y1');
    expect(result?.coefficients[0]?.variable).toBe('x1');
  });

  it('rejects missing is_significant on coefficients', () => {
    expect(
      parseIv2slsFirstStageResult({
        endog_name: 'y1',
        var_names: ['x1'],
        r_squared: 0.1,
        adj_r_squared: 0.05,
        coefficients: [{ variable: 'x1', coef: 1 }],
      }),
    ).toBeNull();
  });
});

const MINIMAL_REGRESSION = {
  title: 'OLS',
  model_basic_info: {
    model_type: 'OLS',
    method: 'Least Squares',
    num_observation: 100,
    r_squared: 0.5,
    adj_r_squared: 0.48,
    f_statistic: 10,
    prob_f_statistic: 0.001,
    df_model: 2,
    df_residual: 97,
    df_total: 99,
    ss_model: 50,
    ss_residual: 50,
    ss_total: 100,
    ms_model: 25,
    ms_residual: 0.5,
    ms_total: 1,
    covariance_type: 'nonrobust',
  },
  coefficients: [{ variable: 'const', coef: 1, is_significant: true }],
  diagnostic_info: { cond_no: 10 },
};

describe('parseRegressionResultData', () => {
  it('accepts minimal OLS-shaped payload', () => {
    expect(parseRegressionResultData(MINIMAL_REGRESSION)?.title).toBe('OLS');
  });

  it('parses backend leverage KDE points', () => {
    const parsed = parseRegressionResultData({
      ...MINIMAL_REGRESSION,
      diagnostic_info: {
        cond_no: 10,
        leverage: [0.1, 0.2],
        leverage_kde: [{ x: 0, y: 1.25 }],
      },
    });

    expect(parsed?.diagnostic_info.leverage_kde).toEqual([{ x: 0, y: 1.25 }]);
  });

  it.each([
    [{ x: Number.NaN, y: 1 }],
    [{ x: 0, y: Number.POSITIVE_INFINITY }],
    [{ x: 0, y: 'bad' }],
  ])('rejects malformed leverage KDE points: %j', leverage_kde => {
    expect(
      parseRegressionResultData({
        ...MINIMAL_REGRESSION,
        diagnostic_info: { cond_no: 10, leverage_kde },
      }),
    ).toBeNull();
  });

  it('rejects missing cond_no', () => {
    expect(
      parseRegressionResultData({ ...MINIMAL_REGRESSION, diagnostic_info: {} }),
    ).toBeNull();
  });
});

describe('parseReportPayload', () => {
  it('dispatches regression reports through shared parser', () => {
    expect(parseReportPayload('olsSummary', MINIMAL_REGRESSION)).not.toBeNull();
  });

  it('rejects panel_did without kind discriminator', () => {
    expect(parseReportPayload('panelDid', { title: 'DID' })).toBeNull();
  });
});
