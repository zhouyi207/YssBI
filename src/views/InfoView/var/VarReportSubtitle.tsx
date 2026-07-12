import type { VARSummaryResultData } from '@/shared/types/report';

export function VarReportSubtitle({
  var_names,
  complete_sample_rows,
  var_max_lag,
  num_observation,
}: Pick<VARSummaryResultData, 'var_names' | 'complete_sample_rows' | 'var_max_lag' | 'num_observation'>) {
  if (complete_sample_rows != null && var_max_lag != null) {
    return (
      <span className="text-xs leading-relaxed text-muted-foreground">
        Variables: {var_names.join(', ')} · T={complete_sample_rows}（时间轴对齐行数）· p={var_max_lag} · n=
        {num_observation}
        （Stata Number of obs；仅内生 listwise 时 n = T − p；有外生 DataFrame 时与 Stata var ex() 相同，仅当期 exog[t] 须有效）
      </span>
    );
  }

  return (
    <span className="text-xs leading-relaxed text-muted-foreground">
      Variables: {var_names.join(', ')} · n={num_observation}
    </span>
  );
}
