export interface ValidationIssueDTO {
  code: string;
  severity: "error" | "warning";
  path: string;
}

export interface ValidationReportDTO {
  ok: boolean;
  errors: ValidationIssueDTO[];
  warnings: ValidationIssueDTO[];
}
