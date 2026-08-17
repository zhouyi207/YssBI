import type { BayesModelDraftDTO, ValidationIssueDTO, ValidationReportDTO } from '@/shared/types/bayes';

export function validateBayesDraftLocally(draft: BayesModelDraftDTO): ValidationReportDTO {
  const errors: ValidationIssueDTO[] = [];
  const warnings: ValidationIssueDTO[] = [];

  if (!draft.dataset) {
    errors.push({ code: 'dataset_required', severity: 'error', path: 'dataset' });
  }
  if (!draft.responseBinding) {
    errors.push({ code: 'response_required', severity: 'error', path: 'responseBinding' });
  }
  if (!draft.formulaText.trim()) {
    errors.push({ code: 'formula_required', severity: 'error', path: 'formulaText' });
  } else if (!draft.rawPredictor) {
    errors.push({ code: 'formula_not_parsed', severity: 'error', path: 'rawPredictor' });
  } else if (!draft.boundPredictor) {
    errors.push({ code: 'predictor_not_bound', severity: 'error', path: 'boundPredictor' });
  }
  for (const symbol of draft.symbols.filter(symbol => symbol.role === 'independent')) {
    if (!draft.dataBindings[symbol.name]) {
      errors.push({ code: 'data_binding_required', severity: 'error', path: `dataBindings.${symbol.name}` });
    }
  }
  for (const symbol of draft.symbols.filter(symbol => symbol.role === 'dependent')) {
    if (draft.responseBinding?.symbol !== symbol.name || !draft.responseBinding.column) {
      errors.push({ code: 'response_binding_required', severity: 'error', path: 'responseBinding' });
    }
  }
  if (draft.parameters.length === 0) {
    warnings.push({ code: 'no_parameters', severity: 'warning', path: 'parameters' });
  }
  if (draft.sampler.samples < 500) {
    warnings.push({ code: 'low_sample_count', severity: 'warning', path: 'sampler.samples' });
  }

  return { ok: errors.length === 0, errors, warnings };
}

export function issueTargetStep(issue: ValidationIssueDTO): string {
  const path = issue.path ?? '';
  if (path.startsWith('dataset') || path.startsWith('response')) return 'data';
  if (path.startsWith('formula') || path.startsWith('rawPredictor') || path.startsWith('boundPredictor')) return 'formula';
  if (path.startsWith('dataBindings')) return 'data';
  if (path.startsWith('likelihood')) return 'likelihood';
  if (path.startsWith('parameters')) return 'parameters';
  if (path.startsWith('sampler')) return 'sampler';
  return 'run';
}
