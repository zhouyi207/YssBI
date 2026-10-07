import type { SerialTestsResponseDTO } from "@/shared/types/report/serialTests";
import { formatNum } from "@/shared/stats/formatStat";

export function SerialTestsResultView({ result }: { result: SerialTestsResponseDTO }) {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 mt-4">
      {result.bg && (
        <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
          <div className="text-[11px] text-muted-foreground font-mono mb-2">Breusch-Godfrey LM</div>
          <div className="text-foreground font-mono text-sm font-medium">
            χ²({result.bg.lags}) = {formatNum(result.bg.stat)}
          </div>
          <div className="text-xs text-muted-foreground mt-1">
            p = {formatNum(result.bg.p_value)}
            {result.bg.p_value < 0.05 ? <span className="text-amber-400 ml-1">*</span> : null}
          </div>
        </div>
      )}
      {result.q && (
        <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
          <div className="text-[11px] text-muted-foreground font-mono mb-2">Ljung-Box Q</div>
          <div className="text-foreground font-mono text-sm font-medium">
            Q({result.q.lags}) = {formatNum(result.q.stat)}
          </div>
          <div className="text-xs text-muted-foreground mt-1">
            p = {formatNum(result.q.p_value)}
            {result.q.p_value < 0.05 ? <span className="text-amber-400 ml-1">*</span> : null}
          </div>
        </div>
      )}
      <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
        <div className="text-[11px] text-muted-foreground font-mono mb-2">Durbin-Watson</div>
        <div className="text-foreground font-mono text-sm font-medium">
          DW = {formatNum(result.dw.d)}
        </div>
      </div>
    </div>
  );
}
