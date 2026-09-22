import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { normalizeSerialTestsResponse } from "./serialTests";
import { parseReportPayloadResult } from "./parseReportPayload";

describe("normalizeSerialTestsResponse", () => {
  it("preserves the named serial-test statistics", () => {
    const response = {
      dw: { d: 1.85 },
      q: { stat: 2.1, p_value: 0.3, lags: 5 },
      bg: { stat: 6.4, p_value: 0.04, lags: 3 },
    };
    expect(normalizeSerialTestsResponse(response)).toEqual(response);
  });

  it("rejects a bare number for dw", () => {
    expect(normalizeSerialTestsResponse({ dw: 1.85 })).toBeNull();
  });

  it("rejects the response when a supplied BG block is malformed", () => {
    expect(
      normalizeSerialTestsResponse({
        dw: { d: 2 },
        bg: { stat: 1, p_value: 0.5 },
      }),
    ).toBeNull();
  });
});

describe("IV first-stage report parsing", () => {
  it("accepts Rust-shaped first stage JSON", () => {
    const firstStage = {
      endog_name: "y1",
      var_names: ["x1", "z1"],
      r_squared: 0.42,
      adj_r_squared: 0.4,
      coefficients: [
        {
          variable: "x1",
          coef: 1.2,
          std_err: 0.3,
          t_value: 4,
          p_value: 0.001,
          is_significant: true,
        },
      ],
    };
    const result = parseReportPayloadResult("iv2slsSummary", {
      ...MINIMAL_REGRESSION,
      title: "2SLS Summary",
      model_basic_info: { ...MINIMAL_REGRESSION.model_basic_info, model_type: "2SLS" },
      diagnostic_info: { cond_no: 10, iv2sls_first_stage: [firstStage] },
    });
    expect(result).toMatchObject({
      ok: true,
      value: { diagnostic_info: { iv2sls_first_stage: [firstStage] } },
    });
  });

  it("rejects missing is_significant on coefficients", () => {
    expect(
      parseReportPayloadResult("iv2slsSummary", {
        ...MINIMAL_REGRESSION,
        title: "2SLS Summary",
        model_basic_info: { ...MINIMAL_REGRESSION.model_basic_info, model_type: "2SLS" },
        diagnostic_info: {
          cond_no: 10,
          iv2sls_first_stage: [
            {
              endog_name: "y1",
              var_names: ["x1"],
              r_squared: 0.1,
              adj_r_squared: 0.05,
              coefficients: [{ variable: "x1", coef: 1 }],
            },
          ],
        },
      }),
    ).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.iv2sls_first_stage[0].coefficients[0].is_significant" },
    });
  });
});

const MINIMAL_REGRESSION = {
  title: "Prais Summary",
  model_basic_info: {
    model_type: "Prais-Winsten",
    method: "GLS",
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
    covariance_type: "nonrobust",
  },
  coefficients: [{ variable: "const", coef: 1, is_significant: true }],
  diagnostic_info: { cond_no: 10 },
};

describe("regression report parsing", () => {
  it("rejects diagnostic arrays that cannot be paired with observations or coefficients", () => {
    const parse = (diagnostics: object) =>
      parseReportPayloadResult("praisSummary", {
        ...MINIMAL_REGRESSION,
        diagnostic_info: { cond_no: 10, ...diagnostics },
      });
    const valid = {
      fitted_values: [1, 2],
      residuals: [0, 0.1],
      residual_scatter: { e: [0.1], e_lag1: [0] },
      exog: [[1], [1]],
      exog_means: [1],
    };
    expect(parse(valid).ok).toBe(true);
    expect(parse({ ...valid, residuals: [0] })).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.residuals" },
    });
    expect(parse({ ...valid, residual_scatter: { e: [0.1], e_lag1: [] } })).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.residual_scatter" },
    });
    expect(parse({ ...valid, exog_means: [] })).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.exog_means" },
    });
    expect(parse({ ...valid, exog: [[1, 2]] })).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.exog" },
    });
  });

  it("requires the statistics for the selected IV test and preserves genuine zero values", () => {
    const parse = (overid: object) =>
      parseReportPayloadResult("iv2slsSummary", {
        ...MINIMAL_REGRESSION,
        diagnostic_info: { cond_no: 10, iv2sls_overid: overid },
      });
    const wooldridge = {
      test_type: "wooldridge",
      wooldridge_stat: 0,
      wooldridge_p_value: 0,
      df: 1,
    };
    expect(parse(wooldridge)).toMatchObject({
      ok: true,
      value: { diagnostic_info: { iv2sls_overid: wooldridge } },
    });
    expect(parse({ ...wooldridge, wooldridge_p_value: undefined })).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.iv2sls_overid.wooldridge_p_value" },
    });
    const classical = {
      test_type: "sargan_basmann",
      sargan_stat: 0,
      sargan_p_value: 1,
      basmann_stat: 0,
      basmann_p_value: 1,
      df: 1,
    };
    expect(parse(classical)).toMatchObject({
      ok: true,
      value: { diagnostic_info: { iv2sls_overid: classical } },
    });
    expect(parse({ ...classical, basmann_stat: undefined })).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.iv2sls_overid.basmann_stat" },
    });
  });

  it("preserves binary model statistics and hypothesis inputs", () => {
    const parsed = parseReportPayloadResult("binarySummary", {
      ...MINIMAL_REGRESSION,
      model_basic_info: {
        model_type: "Logit",
        method: "Maximum Likelihood",
        num_observation: 100,
        pseudo_r2: 0.15,
        adjusted_pseudo_r2: 0.1,
        log_likelihood: -8.5,
        lr_chi2: 3,
        prob_lr_chi2: 0.083,
        df_model: 0,
        df_residual: 99,
        covariance_type: "nonrobust",
        aic: 19,
        bic: 20,
      },
      title: "LOGIT Summary",
      betas: [0.25],
      cov_beta: [[0.0625]],
      model_statistics: {
        kind: "binary",
        link: "logit",
        covariance: [[0.0625]],
        standardErrors: [0.25],
        statisticValues: [1],
        pValues: [0.317],
        confidenceIntervalLower: [-0.24],
        confidenceIntervalUpper: [0.74],
        logLikelihood: -8.5,
        nullLogLikelihood: -10,
        pseudoR2: 0.15,
        adjustedPseudoR2: 0.1,
        lrChi2: 3,
        lrPValue: 0.083,
        aic: 19,
        bic: 20,
        iterations: 6,
        converged: true,
        conditionNumber: 2.5,
      },
    });

    expect(parsed).toMatchObject({
      ok: true,
      value: {
        model_basic_info: { pseudo_r2: 0.15, lr_chi2: 3 },
        betas: [0.25],
        cov_beta: [[0.0625]],
        model_statistics: {
          kind: "binary",
          link: "logit",
          covariance: [[0.0625]],
          logLikelihood: -8.5,
          converged: true,
        },
      },
    });
    expect(parseReportPayloadResult("binarySummary", MINIMAL_REGRESSION)).toMatchObject({
      ok: false,
      issue: { fieldPath: "model_basic_info.pseudo_r2" },
    });
  });

  it("parses backend leverage KDE points", () => {
    const parsed = parseReportPayloadResult("praisSummary", {
      ...MINIMAL_REGRESSION,
      diagnostic_info: {
        cond_no: 10,
        leverage: [0.1, 0.2],
        leverage_kde: [{ x: 0, y: 1.25 }],
      },
    });

    expect(parsed).toMatchObject({
      ok: true,
      value: { diagnostic_info: { leverage_kde: [{ x: 0, y: 1.25 }] } },
    });
  });

  it.each([
    { leverage_kde: [{ x: Number.NaN, y: 1 }], coordinate: "x" },
    { leverage_kde: [{ x: 0, y: Number.POSITIVE_INFINITY }], coordinate: "y" },
    { leverage_kde: [{ x: 0, y: "bad" }], coordinate: "y" },
  ])("rejects malformed leverage KDE points: %j", ({ leverage_kde, coordinate }) => {
    expect(
      parseReportPayloadResult("praisSummary", {
        ...MINIMAL_REGRESSION,
        diagnostic_info: { cond_no: 10, leverage_kde },
      }),
    ).toMatchObject({
      ok: false,
      issue: { fieldPath: `diagnostic_info.leverage_kde[0].${coordinate}` },
    });
  });

  it("rejects missing cond_no", () => {
    expect(
      parseReportPayloadResult("praisSummary", { ...MINIMAL_REGRESSION, diagnostic_info: {} }),
    ).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.cond_no", reason: "missing required field" },
    });
  });
});

describe("parseReportPayloadResult", () => {
  it("rejects malformed nested report fields and preserves nullable VEC statistics", () => {
    const vec = {
      title: "VEC Summary",
      var_names: ["y"],
      num_observation: 10,
      rank: 1,
      lags: 1,
      trend_spec: "constant",
      equations: [],
      coefficients: [],
      beta: [[1]],
      beta_std_err: [[null]],
      cointegrating_equations: [],
      log_likelihood: 1,
      aic: 1,
      hqic: 1,
      sbic: 1,
      det_sigma_ml: 1,
    };
    expect(parseReportPayloadResult("vecSummary", vec)).toEqual({ ok: true, value: vec });
    expect(parseReportPayloadResult("vecSummary", { ...vec, coefficients: [null] })).toMatchObject({
      ok: false,
      issue: { fieldPath: "coefficients[0]" },
    });
    expect(
      parseReportPayloadResult("vecSummary", { ...vec, beta_std_err: [["invalid"]] }),
    ).toMatchObject({
      ok: false,
      issue: { fieldPath: "beta_std_err[0][0]" },
    });
    expect(
      parseReportPayloadResult("praisSummary", {
        ...MINIMAL_REGRESSION,
        diagnostic_info: { cond_no: 1, vif: [null] },
      }),
    ).toMatchObject({
      ok: false,
      issue: { fieldPath: "diagnostic_info.vif[0]" },
    });
    expect(
      parseReportPayloadResult("panelSummary", {
        title: "Panel",
        endog_name: "y",
        selection_tests: [null],
      }),
    ).toMatchObject({ ok: false, issue: { fieldPath: "selection_tests[0]" } });
    expect(
      parseReportPayloadResult("varSoc", {
        title: "Lag selection",
        var_names: ["y"],
        maxlag: 2,
        num_observation: 10,
        rows: [null],
      }),
    ).toMatchObject({ ok: false, issue: { fieldPath: "rows[0]" } });
  });

  it("accepts the Rust Linear Regression Summary report output", () => {
    const payload: unknown = JSON.parse(
      readFileSync(
        resolve("src/tests/fixtures/node-system-contracts/ols-summary-report.json"),
        "utf8",
      ),
    );

    expect(parseReportPayloadResult("linearRegressionSummary", payload)).toEqual({
      ok: true,
      value: payload,
    });
  });

  it("rejects display values and table rows that contradict their declared formats", () => {
    const read = () =>
      JSON.parse(
        readFileSync(
          resolve("src/tests/fixtures/node-system-contracts/ols-summary-report.json"),
          "utf8",
        ),
      );
    const metric = read();
    metric.presentation.summary.items[2].value = "0.875";
    expect(parseReportPayloadResult("linearRegressionSummary", metric)).toMatchObject({
      ok: false,
      issue: { fieldPath: "presentation.summary.items[2].value" },
    });
    const table = read();
    table.presentation.anova.rows[0].pop();
    expect(parseReportPayloadResult("linearRegressionSummary", table)).toMatchObject({
      ok: false,
      issue: { fieldPath: "presentation.anova.rows[0]" },
    });
  });
});
