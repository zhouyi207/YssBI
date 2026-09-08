import type { ValidationIssueDTO } from "@/shared/types/bayes";

export function issueTargetStep(issue: ValidationIssueDTO): string {
  const path = issue.path ?? "";
  if (path.startsWith("dataset") || path.startsWith("response")) return "data";
  if (
    path.startsWith("formula") ||
    path.startsWith("rawPredictor") ||
    path.startsWith("boundPredictor")
  )
    return "formula";
  if (path.startsWith("dataBindings")) return "data";
  if (path.startsWith("likelihood")) return "likelihood";
  if (path.startsWith("parameters")) return "parameters";
  if (path.startsWith("sampler")) return "sampler";
  return "run";
}
