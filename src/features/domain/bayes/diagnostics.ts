import type { DiagnosticWarningDTO, InferenceResultDTO, ParameterSummaryDTO } from '@/shared/types/bayes';

export type DiagnosticSeverity = 'good' | 'warning' | 'bad' | 'unknown';
export type ParameterDiagnosticStatus = 'ok' | 'check_rhat' | 'low_ess' | 'unknown';

export interface DiagnosticAssessment {
  severity: DiagnosticSeverity;
  title: string;
  summary: string;
  suggestions: string[];
  metrics: DiagnosticMetric[];
  warnings: DiagnosticWarningDescription[];
}

export interface DiagnosticMetric {
  key: 'sampling' | 'rhat' | 'ess' | 'divergences' | 'max_treedepth_hits';
  label: string;
  severity: DiagnosticSeverity;
}

export interface DiagnosticWarningDescription {
  code: string;
  title: string;
  explanation: string;
  suggestion: string;
  parameter?: string;
}

const RHAT_WARNING_THRESHOLD = 1.01;
const RHAT_BAD_THRESHOLD = 1.1;
const MIN_ESS = 100;

export function evaluateInferenceDiagnostics(result: InferenceResultDTO | null): DiagnosticAssessment {
  if (!result) {
    return {
      severity: 'unknown',
      title: 'No result yet',
      summary: '运行完成后显示 MCMC 诊断状态。',
      suggestions: [],
      metrics: [],
      warnings: [],
    };
  }

  const summaries = result.summaries;
  const warnings = result.diagnostics.warnings ?? [];
  const missingDiagnostics = summaries.some(summary => summary.rhat == null || summary.essBulk == null || summary.essTail == null);
  const hasBadRhat = summaries.some(summary => (summary.rhat ?? 0) > RHAT_BAD_THRESHOLD);
  const hasWarningRhat = summaries.some(summary => (summary.rhat ?? 0) > RHAT_WARNING_THRESHOLD);
  const hasLowEss = summaries.some(summary => isLowEss(summary.essBulk) || isLowEss(summary.essTail));
  const hasDivergences = (result.diagnostics.divergences ?? 0) > 0;
  const hasTreedepthHits = (result.diagnostics.maxTreedepthHits ?? 0) > 0;
  const hasBackendWarning = warnings.length > 0;
  const details = diagnosticDetails(result, {
    hasBadRhat,
    hasWarningRhat,
    hasLowEss,
    missingDiagnostics,
  });

  if (hasBadRhat || hasDivergences) {
    return {
      severity: 'bad',
      title: 'Diagnostics need attention',
      summary: hasDivergences
        ? '采样存在 divergence，当前后验结果可能不可靠。'
        : '至少一个参数的 R-hat 明显偏高，链之间可能没有充分混合。',
      suggestions: convergenceSuggestions(),
      ...details,
    };
  }
  if (hasWarningRhat || hasLowEss || hasTreedepthHits || hasBackendWarning) {
    return {
      severity: 'warning',
      title: 'Diagnostics warning',
      summary: hasTreedepthHits
        ? '采样达到最大树深度，部分转移可能没有充分探索后验。'
        : hasBackendWarning
          ? '后端报告了诊断 warning，请检查详情后再使用结果。'
          : '采样已经完成，但部分参数的 R-hat 或有效样本量提示结果可能不够稳定。',
      suggestions: convergenceSuggestions(),
      ...details,
    };
  }
  if (missingDiagnostics) {
    return {
      severity: 'unknown',
      title: 'Diagnostics incomplete',
      summary: '结果缺少部分 R-hat 或 ESS 指标，无法完整判断采样质量。',
      suggestions: ['确认后端返回了 MCMCChains 诊断指标。', '如果没有保存 samples，请重新运行并保存 samples。'],
      ...details,
    };
  }
  return {
    severity: 'good',
    title: 'Diagnostics look good',
    summary: '所有参数的基础 R-hat / ESS 诊断都在当前阈值内。',
    suggestions: ['仍建议结合 trace、density、autocorrelation 和 posterior predictive 检查模型是否符合业务预期。'],
    ...details,
  };
}

export function parameterDiagnosticStatus(summary: ParameterSummaryDTO): ParameterDiagnosticStatus {
  if (summary.rhat == null || summary.essBulk == null || summary.essTail == null) return 'unknown';
  if (summary.rhat > RHAT_WARNING_THRESHOLD) return 'check_rhat';
  if (isLowEss(summary.essBulk) || isLowEss(summary.essTail)) return 'low_ess';
  return 'ok';
}

export function parameterDiagnosticLabel(status: ParameterDiagnosticStatus): string {
  switch (status) {
    case 'ok':
      return 'OK';
    case 'check_rhat':
      return 'Check R-hat';
    case 'low_ess':
      return 'Low ESS';
    case 'unknown':
      return 'Unknown';
  }
}

export function describeDiagnosticWarning(warning: DiagnosticWarningDTO): DiagnosticWarningDescription {
  const parameterPrefix = warning.parameter ? `参数 ${warning.parameter}: ` : '';
  switch (warning.code) {
    case 'RHAT_TOO_HIGH':
      return {
        code: warning.code,
        parameter: warning.parameter,
        title: `${parameterPrefix}R-hat 偏高`,
        explanation: '不同链之间没有充分混合，当前后验摘要可能不稳定。',
        suggestion: '增加 warmup / samples，检查模型是否过参数化，或收紧不合理的宽 prior。',
      };
    case 'ESS_TOO_LOW':
      return {
        code: warning.code,
        parameter: warning.parameter,
        title: `${parameterPrefix}有效样本量不足`,
        explanation: '独立有效样本较少，均值、分位数和可信区间可能有较大 Monte Carlo 误差。',
        suggestion: '增加 samples，检查自相关；必要时标准化 predictor 或调整模型参数化。',
      };
    case 'JULIA_BAYES_TURING_LINEAR_POC':
    case 'JULIA_BAYES_TURING_GENERIC_NORMAL':
    case 'JULIA_BAYES_TURING_GENERIC_BERNOULLI_LOGIT':
    case 'JULIA_BAYES_TURING_GENERIC_POISSON_LOG':
      return {
        code: warning.code,
        parameter: warning.parameter,
        title: 'Julia backend executed',
        explanation: '当前模型已由 Julia/Turing 后端完成采样。',
        suggestion: '继续检查采样诊断和 posterior predictive，而不是只看采样是否成功。',
      };
    default:
      return {
        code: warning.code,
        parameter: warning.parameter,
        title: warning.code,
        explanation: warning.message || '后端返回了诊断信息。',
        suggestion: '查看原始 warning message，并结合结果图表判断是否需要调整模型。',
      };
  }
}

export function diagnosticSeverityClass(severity: DiagnosticSeverity): string {
  switch (severity) {
    case 'good':
      return 'text-emerald-500';
    case 'warning':
      return 'text-amber-500';
    case 'bad':
      return 'text-destructive';
    case 'unknown':
      return 'text-muted-foreground';
  }
}

function diagnosticDetails(
  result: InferenceResultDTO,
  flags: { hasBadRhat: boolean; hasWarningRhat: boolean; hasLowEss: boolean; missingDiagnostics: boolean },
): Pick<DiagnosticAssessment, 'metrics' | 'warnings'> {
  const diagnostics = result.diagnostics;
  const rhatSeverity: DiagnosticSeverity = flags.missingDiagnostics ? 'unknown' : flags.hasBadRhat ? 'bad' : flags.hasWarningRhat ? 'warning' : 'good';
  const essSeverity: DiagnosticSeverity = flags.missingDiagnostics ? 'unknown' : flags.hasLowEss ? 'warning' : 'good';
  return {
    metrics: [
      { key: 'sampling', label: `Chains: ${diagnostics.chains}, draws per chain: ${diagnostics.drawsPerChain}, warmup: ${diagnostics.warmup}`, severity: 'good' },
      { key: 'rhat', label: flags.missingDiagnostics ? 'R-hat: incomplete' : 'R-hat: evaluated for all parameters', severity: rhatSeverity },
      { key: 'ess', label: flags.missingDiagnostics ? 'ESS: incomplete' : 'Bulk and tail ESS: evaluated for all parameters', severity: essSeverity },
      { key: 'divergences', label: `Divergences: ${diagnostics.divergences ?? 0}`, severity: (diagnostics.divergences ?? 0) > 0 ? 'bad' : 'good' },
      {
        key: 'max_treedepth_hits',
        label: diagnostics.maxTreedepthHits == null
          ? 'Max treedepth hits: unavailable'
          : `Max treedepth hits: ${diagnostics.maxTreedepthHits}`,
        severity: diagnostics.maxTreedepthHits == null
          ? 'unknown'
          : diagnostics.maxTreedepthHits > 0 ? 'warning' : 'good',
      },
    ],
    warnings: (diagnostics.warnings ?? []).map(describeDiagnosticWarning),
  };
}

function isLowEss(value: number | undefined): boolean {
  return value != null && value < MIN_ESS;
}

function convergenceSuggestions(): string[] {
  return [
    '增加 samples，例如从 2,000 提高到 4,000 或更多。',
    '增加 warmup，给 NUTS 更多适应时间。',
    '检查 prior 是否过宽或与数据尺度不匹配。',
    '考虑标准化 predictor，减少参数之间的强相关。',
    '优先查看 trace 和 autocorrelation，确认链是否混合良好。',
    '若存在 divergence 或 treedepth hit，考虑提高 target accept 或 max tree depth。',
  ];
}
