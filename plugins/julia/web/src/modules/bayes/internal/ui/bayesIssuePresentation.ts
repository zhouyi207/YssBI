import type { TFunction } from "i18next";
import {
  type BayesArtifactsModel,
  type BayesApplicationError,
  type BayesInferenceError,
} from "@/features/application/bayes";
import type { DiagnosticWarningDTO, ValidationIssueDTO } from "@/shared/types/bayes";

type BayesErrorReference = NonNullable<BayesArtifactsModel["issue"]>;

export function bayesErrorMessage(error: BayesApplicationError, t: TFunction): string {
  return t(`bayes.errors.${error.code}`, {
    ...error.details,
    code: error.code,
    defaultValue: t("bayes.errors.unknown", { code: error.code }),
  });
}

export function bayesInferenceErrorMessage(error: BayesInferenceError, t: TFunction): string {
  return bayesErrorMessage(error, t);
}

export function bayesErrorReferenceMessage(error: BayesErrorReference, t: TFunction): string {
  const message = bayesErrorMessage(
    {
      code: error.code,
      details: null,
      incidentId: error.incidentId,
    },
    t,
  );
  return error.incidentId ? `${message} · ${t("common.incidentId")}: ${error.incidentId}` : message;
}

export function bayesValidationIssueMessage(issue: ValidationIssueDTO, t: TFunction): string {
  return t(`bayes.validation.issues.${issue.code}`, {
    code: issue.code,
    path: issue.path,
    defaultValue: t("bayes.validation.unknown", { code: issue.code }),
  });
}

export function bayesDiagnosticWarningText(
  warning: DiagnosticWarningDTO,
  part: "title" | "explanation" | "suggestion",
  t: TFunction,
): string {
  return t(`bayes.results.diagnostics.warnings.${warning.code}.${part}`, {
    parameter: warning.parameter,
    code: warning.code,
    defaultValue: t(`bayes.results.diagnostics.warnings.unknown.${part}`, {
      code: warning.code,
      parameter: warning.parameter,
    }),
  });
}
