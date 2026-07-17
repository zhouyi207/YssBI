export interface ValidationIssueDTO {
  code: string;
  severity: 'error' | 'warning';
  message: string;
  path?: string;
  hint?: string;
}

export interface ValidationReportDTO {
  ok: boolean;
  errors: ValidationIssueDTO[];
  warnings: ValidationIssueDTO[];
}

export interface BayesValidationStateDTO {
  draftHash: string;
  report: ValidationReportDTO | null;
}

export const EMPTY_VALIDATION_REPORT: ValidationReportDTO = {
  ok: true,
  errors: [],
  warnings: [],
};
