import { formatNum } from './RegressionShared';
import type { ModelBasicInfo } from '@/shared/types/report';

/** MLE iteration log — separate module at bottom (Stata-style) */
export function PanelMLEIterationBlock({ info }: { info: ModelBasicInfo }) {
  const hasConst = info.mle_iter_log_lik_const != null && info.mle_iter_log_lik_const.length > 0;
  const hasFull = info.mle_iter_log_lik != null && info.mle_iter_log_lik.length > 0;
  if (!hasConst && !hasFull) return null;

  return (
    <div className="rounded-lg border border-border bg-card overflow-hidden">
      <div className="px-4 py-2.5 border-b border-border">
        <span className="text-[11px] text-muted-foreground uppercase tracking-wider font-medium">
          MLE Iteration Log
        </span>
      </div>
      <div className="px-4 py-3 space-y-4">
        {hasConst && (
          <div>
            <span className="text-muted-foreground text-xs block mb-1.5">Fitting constant-only model:</span>
            <div className="text-foreground text-xs font-mono space-y-0.5">
              {info.mle_iter_log_lik_const!.map((ll, i) => (
                <div key={i}>
                  Iteration {i}: Log likelihood = {formatNum(ll)}
                </div>
              ))}
            </div>
          </div>
        )}
        {hasFull && (
          <div>
            <span className="text-muted-foreground text-xs block mb-1.5">Fitting full model:</span>
            <div className="text-foreground text-xs font-mono space-y-0.5">
              {info.mle_iter_log_lik!.map((ll, i) => (
                <div key={i}>
                  Iteration {i}: Log likelihood = {formatNum(ll)}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
