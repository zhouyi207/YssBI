import { useState, type FC, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useOlsReport } from "@/features/application/stats/useOlsReport";
import { useResultAnalysisQuery } from "@/features/application/results/useResultAnalysis";
import {
  usePagedResultRows,
  type PagedResultRowsState,
} from "@/features/application/results/usePagedResultRows";
import { ReadOnlyDataGrid } from "@/features/application/results/components/ReadOnlyDataGrid";
import { ResultPageToolbar } from "@/features/application/results/components/ResultPageToolbar";
import { ResultReadError } from "@/features/application/results/components/ResultReadError";
import { ScatterChart } from "@/shared/charts/cartesian/ScatterChart";
import type { OlsReportData } from "@/shared/types/domain/resultReport";
import type { ResultReference } from "@/shared/types/domain/result";
import {
  ReportLayout,
  ReportLazyBoundary,
  ReportSection,
  RSquaredBadge,
  LazyFormulaBlock,
} from "./shared";
import { ModelSummaryGrid } from "./shared/ModelSummaryGrid";
import { AnovaTable } from "./shared/AnovaTable";
import { CoefficientsBlock } from "./shared/CoefficientsBlock";
import { HypothesisTestBlock } from "./shared/HypothesisTestBlock";
import { ACFPACFBlock } from "./shared/ACFPACFBlock";
import { SerialTestsBlock } from "./shared/SerialTestsBlock";

function PageToolbar({ page }: { page: PagedResultRowsState }) {
  return (
    <ResultPageToolbar {...page} onPrevious={page.goToPreviousPage} onNext={page.goToNextPage} />
  );
}

function ExpandableReportSection({ title, children }: { title: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <details
      className="my-5 rounded-lg border border-border p-4"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary className="cursor-pointer text-sm font-medium">{title}</summary>
      {open && <div className="mt-4">{children}</div>}
    </details>
  );
}

function OlsObservations({ data }: { data: OlsReportData }) {
  const page = usePagedResultRows(
    data.resultRef,
    data.observations.rowCount,
    200,
    undefined,
    data.observations.part,
  );
  return (
    <div className="space-y-3">
      {page.error ? (
        <ResultReadError error={page.error} onRetry={() => void page.reload()} />
      ) : null}
      <ReadOnlyDataGrid
        columns={[...page.columns]}
        rows={page.rows.map((row) => [...row])}
        loading={page.loading}
        pageStartIndex={page.offset}
        height={320}
      />
      <PageToolbar page={page} />
    </div>
  );
}

function OlsResidualPlot({ reference }: { reference: ResultReference }) {
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [xRange, setXRange] = useState<[number, number] | undefined>();
  const query = useResultAnalysisQuery(reference, {
    kind: "residualPlot",
    maxPoints: 2000,
    xRange,
  });
  const plot = query.value?.kind === "residualPlot" ? query.value.value : null;
  const validRange =
    min.trim() !== "" &&
    max.trim() !== "" &&
    Number.isFinite(Number(min)) &&
    Number.isFinite(Number(max)) &&
    Number(min) <= Number(max);
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Input
          aria-label="Minimum fitted value"
          type="number"
          value={min}
          onChange={(event) => setMin(event.target.value)}
          placeholder="Min fitted"
          className="w-32"
        />
        <Input
          aria-label="Maximum fitted value"
          type="number"
          value={max}
          onChange={(event) => setMax(event.target.value)}
          placeholder="Max fitted"
          className="w-32"
        />
        <Button
          size="sm"
          variant="outline"
          disabled={!validRange || query.loading}
          onClick={() => setXRange([Number(min), Number(max)])}
        >
          Apply range
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            setXRange(undefined);
            setMin("");
            setMax("");
          }}
        >
          Reset
        </Button>
      </div>
      {query.error ? (
        <ResultReadError error={query.error} onRetry={() => void query.reload()} />
      ) : null}
      {query.loading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : (
        plot && (
          <>
            <p className="text-xs text-muted-foreground">
              {plot.sampled
                ? `Showing ${plot.points.length} sampled observations of ${plot.matchedCount}`
                : `${plot.matchedCount} observations`}
              {plot.matchedCount !== plot.totalCount ? ` (${plot.totalCount} total)` : ""}
            </p>
            <ScatterChart
              data={plot.points.map((point) => ({ x: point.x, y: point.y }))}
              xAxis={{ label: "Fitted Values", valueType: "number" }}
              yAxis={{ label: "Residuals", valueType: "number" }}
              height={280}
              symmetricY
              zeroLine
            />
          </>
        )
      )}
    </div>
  );
}

export const OLSComponent: FC<{ data: OlsReportData }> = ({ data }) => (
  <OlsReport key={`${data.resultRef.executionSessionId}:${data.resultRef.resultId}`} data={data} />
);

function OlsReport({ data }: { data: OlsReportData }) {
  const { page, coefficients, coefficientError, hypothesisSource, computeAcf, computeSerialTests } =
    useOlsReport(data);
  const info = data.model_basic_info;
  return (
    <ReportLayout
      title={data.title}
      badges={
        <>
          <RSquaredBadge value={info.r_squared} />
          <span className="text-xs text-muted-foreground">
            {info.method} &middot; n={info.num_observation}
          </span>
        </>
      }
    >
      {coefficients.length === data.coefficients.rowCount && coefficients.length > 0 && (
        <ReportSection title="Equation" icon="equation">
          <ReportLazyBoundary variant="formula">
            <LazyFormulaBlock endogName={data.endog_name} coefficients={coefficients} />
          </ReportLazyBoundary>
        </ReportSection>
      )}
      <ReportSection title="Model Summary" icon="modelSummary">
        <ModelSummaryGrid info={info} />
      </ReportSection>
      <ReportSection title="ANOVA" icon="anova">
        <AnovaTable info={info} />
      </ReportSection>
      {page.error || coefficientError ? (
        <ResultReadError
          error={(page.error ?? coefficientError)!}
          onRetry={() => void page.reload()}
        />
      ) : null}
      {page.loading ? (
        <p className="text-sm text-muted-foreground">Loading coefficients…</p>
      ) : (
        <CoefficientsBlock coefficients={coefficients} hasCategorical={false} />
      )}
      <PageToolbar page={page} />
      <HypothesisTestBlock source={hypothesisSource} />
      <ReportSection title="Diagnostics" icon="test">
        <p className="text-sm text-muted-foreground">
          Condition number: {data.diagnostic_info.cond_no}
        </p>
        <ExpandableReportSection title="Residuals vs Fitted">
          <OlsResidualPlot reference={data.resultRef} />
        </ExpandableReportSection>
        <ExpandableReportSection title="Fitted values and residuals">
          <OlsObservations data={data} />
        </ExpandableReportSection>
        <ACFPACFBlock observationCount={data.observations.rowCount} compute={computeAcf} />
        <SerialTestsBlock
          observationCount={data.observations.rowCount}
          compute={computeSerialTests}
        />
      </ReportSection>
    </ReportLayout>
  );
}
