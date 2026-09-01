import { createInstance, type TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import { enUS } from "@/app/i18n/locales/en-US";
import { zhCN } from "@/app/i18n/locales/zh-CN";
import { normalizeIpcError } from "@/services/ipc";
import {
  bayesActionErrorMessage,
  bayesDiagnosticWarningText,
  bayesErrorMessage,
  bayesValidationIssueMessage,
} from "./bayesIssuePresentation";

const ERROR_CODES = [
  "bayes_artifact_export_failed",
  "bayes_artifact_export_unsupported",
  "bayes_artifact_not_found",
  "bayes_backend_not_configured",
  "bayes_cancel_failed",
  "bayes_dataset_load_failed",
  "bayes_dataset_source_unsupported",
  "bayes_expression_parse_failed",
  "bayes_inference_failed",
  "bayes_input_bernoulli_response_invalid",
  "bayes_input_column_missing",
  "bayes_input_column_not_numeric",
  "bayes_input_empty",
  "bayes_input_poisson_response_negative",
  "bayes_input_poisson_response_not_integer",
  "bayes_input_predictor_non_finite",
  "bayes_input_response_binding_invalid",
  "bayes_input_response_division_by_zero",
  "bayes_input_response_ln_domain",
  "bayes_input_response_non_finite",
  "bayes_input_response_parameter_forbidden",
  "bayes_input_response_result_non_finite",
  "bayes_input_response_sqrt_domain",
  "bayes_posterior_predictive_invalid",
  "bayes_posterior_predictive_not_found",
  "bayes_request_failed",
  "bayes_result_artifact_read_failed",
  "bayes_result_not_found",
  "bayes_samples_invalid",
  "bayes_samples_not_found",
  "bayes_service_lock_poisoned",
  "bayes_task_active",
  "bayes_task_not_found",
  "bayes_validation_failed",
  "bayes_validation_request_failed",
  "julia_bayes_backend_failed",
  "julia_bayes_invalid_data",
  "julia_bayes_model_unsupported",
  "julia_bayes_package_unavailable",
  "julia_bayes_result_invalid",
  "julia_bayes_result_missing",
  "julia_bayes_runtime_unavailable",
  "julia_bayes_sampling_failed",
] as const;

const VALIDATION_CODES = [
  "data_binding_required",
  "data_column_unknown",
  "dataset_required",
  "dependent_symbol_required",
  "expression_function_arity_invalid",
  "expression_number_invalid",
  "formula_not_parsed",
  "formula_required",
  "likelihood_response_transform_unsupported",
  "likelihood_response_type_invalid",
  "likelihood_sigma_constraint_warning",
  "likelihood_sigma_parameter_required",
  "low_sample_count",
  "no_parameters",
  "parameter_bounds_invalid",
  "parameter_name_duplicated",
  "parameter_name_required",
  "parameter_prior_args_invalid",
  "parameter_prior_constraint_mismatch",
  "poisson_response_non_negative_unchecked",
  "predictor_column_type_invalid",
  "predictor_data_symbol_unconfigured",
  "predictor_not_bound",
  "predictor_parameter_unconfigured",
  "predictor_required",
  "response_binding_mismatch",
  "response_binding_required",
  "response_column_unknown",
  "response_data_symbol_count_invalid",
  "response_expression_required",
  "response_parameter_forbidden",
  "response_required",
  "response_symbol_required",
  "sampler_chains_invalid",
  "sampler_max_tree_depth_invalid",
  "sampler_samples_invalid",
  "sampler_target_accept_invalid",
] as const;

async function translator(locale: typeof enUS | typeof zhCN, language: string): Promise<TFunction> {
  const instance = createInstance();
  await instance.init({
    lng: language,
    fallbackLng: false,
    interpolation: { escapeValue: false },
    resources: { [language]: { translation: locale } },
  });
  return instance.t.bind(instance);
}

describe("Bayes issue presentation inventory", () => {
  it("maps every audited production code in both locales", () => {
    for (const locale of [enUS, zhCN]) {
      for (const code of ERROR_CODES) expect(locale.bayes.errors).toHaveProperty(code);
      for (const code of VALIDATION_CODES)
        expect(locale.bayes.validation.issues).toHaveProperty(code);
      expect(locale.bayes.results.diagnostics.warnings).toHaveProperty("rhat_too_high");
      expect(locale.bayes.results.diagnostics.warnings).toHaveProperty("ess_too_low");
    }
  });

  it.each([
    ["en-US", enUS],
    ["zh-CN", zhCN],
  ] as const)(
    "uses localized generic text plus code for unknown %s issues",
    async (language, locale) => {
      const t = await translator(locale, language);
      const errorText = bayesErrorMessage(
        {
          code: "backend_specific_failure",
          details: null,
          incidentId: null,
        },
        t,
      );
      const validationText = bayesValidationIssueMessage(
        {
          code: "backend_specific_validation",
          severity: "error",
          path: "parameters.beta",
        },
        t,
      );
      const warningText = bayesDiagnosticWarningText(
        {
          code: "backend_specific_warning",
          metric: "ess_bulk",
          value: 20,
          threshold: 100,
          parameter: "beta",
        },
        "title",
        t,
      );

      expect(errorText).toContain("backend_specific_failure");
      expect(validationText).toContain("backend_specific_validation");
      expect(warningText).toContain("backend_specific_warning");
    },
  );

  it("never presents raw IpcError details or raw Error text", async () => {
    const t = await translator(enUS, "en-US");
    const ipcError = normalizeIpcError("export_bayes_artifact_csv", {
      code: "bayes_artifact_export_failed",
      details: { detail: "private backend detail" },
      incidentId: "incident-export-42",
    });

    const ipcText = bayesActionErrorMessage(ipcError, t);
    expect(ipcText).toContain("incident-export-42");
    expect(ipcText).not.toContain("private backend detail");
    expect(bayesActionErrorMessage(new Error("private transport failure"), t)).not.toContain(
      "private transport failure",
    );
  });
});
