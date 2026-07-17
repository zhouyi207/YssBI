import type { BayesModelDraftDTO, ValidationIssueDTO, ValidationReportDTO } from '@/shared/types/bayes';

export function validateBayesDraftLocally(draft: BayesModelDraftDTO): ValidationReportDTO {
  const errors: ValidationIssueDTO[] = [];
  const warnings: ValidationIssueDTO[] = [];

  if (!draft.dataset) {
    errors.push({ code: 'DATASET_REQUIRED', severity: 'error', message: '请选择数据源。', path: 'dataset' });
  }
  if (!draft.responseBinding) {
    errors.push({ code: 'RESPONSE_REQUIRED', severity: 'error', message: '请选择响应变量列。', path: 'responseBinding' });
  }
  if (!draft.formulaText.trim()) {
    errors.push({ code: 'FORMULA_REQUIRED', severity: 'error', message: '请输入模型方程，例如 y = a * x + b。', path: 'formulaText' });
  } else if (!draft.rawPredictor) {
    errors.push({ code: 'FORMULA_NOT_PARSED', severity: 'error', message: '模型方程尚未解析为 raw predictor。', path: 'rawPredictor' });
  } else if (!draft.boundPredictor) {
    errors.push({ code: 'PREDICTOR_NOT_BOUND', severity: 'error', message: '预测项尚未完成符号角色确认。', path: 'boundPredictor' });
  }
  for (const symbol of draft.symbols.filter(symbol => symbol.role === 'independent')) {
    if (!draft.dataBindings[symbol.name]) {
      errors.push({ code: 'DATA_BINDING_REQUIRED', severity: 'error', message: `自变量 ${symbol.name} 尚未绑定数据库列。`, path: `dataBindings.${symbol.name}` });
    }
  }
  for (const symbol of draft.symbols.filter(symbol => symbol.role === 'dependent')) {
    if (draft.responseBinding?.symbol !== symbol.name || !draft.responseBinding.column) {
      errors.push({ code: 'RESPONSE_BINDING_REQUIRED', severity: 'error', message: `因变量 ${symbol.name} 尚未绑定数据库列。`, path: 'responseBinding' });
    }
  }
  if (draft.parameters.length === 0) {
    warnings.push({ code: 'NO_PARAMETERS', severity: 'warning', message: '当前模型尚未识别出未知参数。', path: 'parameters' });
  }
  if (draft.sampler.samples < 500) {
    warnings.push({ code: 'LOW_SAMPLE_COUNT', severity: 'warning', message: '采样数较少，后验诊断可能不稳定。', path: 'sampler.samples' });
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
