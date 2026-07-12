import { Input } from '@/components/ui/input';
import { useHypothesisTestBlock } from '@/features/application/stats/useHypothesisTestBlock';
import { InfoAccentButton } from './InfoViewControls';
import { ReportSection } from './ReportLayout';
import { formatNum } from './RegressionShared';
import { linearFormToLatex, renderHypothesisLatex } from './utils';
import type { RegressionResultData } from '@/shared/types/report';

function HypothesisFormulas({
  form,
  paramNames,
  className = '',
}: {
  form: string;
  paramNames: string[];
  className?: string;
}) {
  const parts = form
    .split(' ; ')
    .map((s) => s.trim())
    .filter(Boolean);
  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {parts.map((part, i) => {
        const html = renderHypothesisLatex(linearFormToLatex(part, paramNames));
        return (
          <div
            key={i}
            className="[&_.katex]:block [&_.katex]:text-xs [&_.katex]:text-foreground"
            dangerouslySetInnerHTML={{ __html: html ?? part }}
          />
        );
      })}
    </div>
  );
}

export function HypothesisTestBlock({ data }: { data: RegressionResultData }) {
  const { hypothesis, setHypothesis, result, error, loading, paramNames, canRun, run } =
    useHypothesisTestBlock(data);

  return (
    <div className="mt-6">
      <ReportSection title="Hypothesis Test (t / Wald)" icon="test">
        <div className="space-y-3 rounded-lg border border-border bg-card p-4">
          <div className="flex gap-2">
            <Input
              type="text"
              value={hypothesis}
              onChange={(e) => setHypothesis(e.target.value)}
              placeholder="e.g. x1 = 0 或 petal_width = -0.5626, petal_length = 0.7（逗号分隔多约束）"
              className="flex-1 font-mono text-sm"
              onKeyDown={(e) => e.key === 'Enter' && void run()}
            />
            <InfoAccentButton onClick={() => void run()} disabled={!canRun} loading={loading}>
              Run
            </InfoAccentButton>
          </div>
          <div className="text-[10px] text-muted-foreground">Param names: {paramNames.join(', ')}</div>
          {error ? <div className="font-mono text-xs text-red-400">{error}</div> : null}
          {result ? (
            <div className="overflow-hidden rounded-md border border-border bg-muted">
              <div className="grid grid-cols-2 divide-x divide-border">
                <div className="min-w-0 p-4">
                  <div className="mb-2 text-[10px] uppercase tracking-wider text-muted-foreground">H₀ 原假设</div>
                  <HypothesisFormulas form={result.h0_form} paramNames={paramNames} />
                </div>
                <div className="min-w-0 p-4">
                  <div className="mb-2 text-[10px] uppercase tracking-wider text-muted-foreground">H₁ 备择假设</div>
                  <HypothesisFormulas form={result.h1_form} paramNames={paramNames} />
                </div>
              </div>
              <div className="space-y-1.5 border-t border-border px-4 py-3 text-xs">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">
                    {result.test_type === 't' ? 't-statistic' : 'F-statistic'}
                  </span>
                  <span className="font-mono text-foreground">{formatNum(result.stat, 4)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">df</span>
                  <span className="font-mono text-muted-foreground">
                    {result.df1}, {result.df2}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">p-value</span>
                  <span
                    className={`font-mono font-medium ${result.p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}
                  >
                    {formatNum(result.p_value, 4)}
                  </span>
                </div>
              </div>
            </div>
          ) : null}
        </div>
      </ReportSection>
    </div>
  );
}
