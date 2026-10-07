import {
  createContext,
  lazy,
  memo,
  Suspense,
  useContext,
  useEffect,
  useMemo,
  useState,
  type FC,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  useLinearRegressionCoefficients,
  LINEAR_COEFFICIENT_PAGE_SIZE,
} from "@/features/application/stats/useLinearRegressionReport";
import { useResultAnalysisQuery } from "@/features/application/results/useResultAnalysis";
import {
  usePagedResultRows,
  type PagedResultRowsState,
} from "@/features/application/results/usePagedResultRows";
import { ReadOnlyDataGrid } from "@/features/application/results/components/ReadOnlyDataGrid";
import { ResultPageToolbar } from "@/features/application/results/components/ResultPageToolbar";
import { ResultReadError } from "@/features/application/results/components/ResultReadError";
import { StructuredData } from "@/components/ui-presentation/StructuredData";
import { Section } from "@/components/ui-presentation/Section";
import { Chart } from "@/components/ui-presentation/Chart";
import type { ChartModel } from "@/shared/charts/ChartModel";
import {
  LINEAR_SUMMARY_CONTENTS,
  type LinearRegressionReportData,
} from "@/shared/types/domain/resultReport";
import type { ResultReference } from "@/shared/types/domain/result";
import { KeyValue } from "@/components/ui-presentation/KeyValue";
import { DataTable } from "@/components/ui-presentation/DataTable";
import { StatCard } from "@/components/ui-presentation/StatCard";
import { CoefficientTable } from "@/components/ui-presentation/CoefficientTable";
import { HypothesisTestResultView } from "./shared/HypothesisTestBlock";
import { AcfPacfResultView } from "./shared/ACFPACFBlock";
import { SerialTestsResultView } from "./shared/SerialTestsBlock";
import { useLinearSummaryContents } from "@/features/application/results/useLinearSummaryContents";
import { AddReportContents } from "./AddReportContents";

const LazyEquation = lazy(() => import("@/components/ui-presentation/Equation"));

function PageToolbar({ page }: { page: PagedResultRowsState }) {
  return (
    <ResultPageToolbar {...page} onPrevious={page.goToPreviousPage} onNext={page.goToNextPage} />
  );
}

function LinearObservations({ data }: { data: LinearRegressionReportData }) {
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
        columns={page.columns}
        rows={page.rows}
        loading={page.loading}
        pageStartIndex={page.offset}
        height={320}
      />
      <PageToolbar page={page} />
    </div>
  );
}

const FITTED_AXIS = { label: "Fitted Values", valueType: "number" } as const;
const RESIDUAL_AXIS = { label: "Residuals", valueType: "number" } as const;

function LinearResidualPlot({ reference }: { reference: ResultReference }) {
  const [adjacent, setAdjacent] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [xRange, setXRange] = useState<[number, number] | undefined>();
  const query = useResultAnalysisQuery(reference, {
    kind: "residualPlot",
    maxPoints: 2000,
    xRange,
    adjacent,
    highlightTopPercent: highlight || undefined,
  });
  const plot = query.value?.kind === "residualPlot" ? query.value.value : null;
  const model = useMemo<ChartModel>(
    () => ({
      kind: "scatter",
      points: plot?.points.map((point) => ({ x: point.x, y: point.y })) ?? [],
      xAxis: adjacent ? { label: "Previous residual", valueType: "number" } : FITTED_AXIS,
      highlightIndices:
        plot?.points.flatMap((point, index) => (point.highlighted ? [index] : [])) ?? [],
      yAxis: RESIDUAL_AXIS,
      symmetricY: true,
      zeroLine: true,
    }),
    [plot?.points, adjacent],
  );
  const validRange =
    min.trim() !== "" &&
    max.trim() !== "" &&
    Number.isFinite(Number(min)) &&
    Number.isFinite(Number(max)) &&
    Number(min) <= Number(max);
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            setAdjacent(!adjacent);
            setXRange(undefined);
            setMin("");
            setMax("");
          }}
        >
          {adjacent ? "Residuals vs previous residual" : "Residuals vs fitted"}
        </Button>
        <label className="flex items-center gap-2 text-sm">
          Highlight top leverage (%)
          <Input
            className="w-20"
            type="number"
            min={0}
            max={100}
            disabled={query.loading || !plot?.highlightAvailable}
            value={highlight}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value) && value >= 0 && value <= 100) setHighlight(value);
            }}
          />
        </label>
        <Input
          aria-label="Minimum horizontal value"
          type="number"
          value={min}
          onChange={(event) => setMin(event.target.value)}
          placeholder="Min x"
          className="w-32"
        />
        <Input
          aria-label="Maximum horizontal value"
          type="number"
          value={max}
          onChange={(event) => setMax(event.target.value)}
          placeholder="Max x"
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
      {plot && !plot.highlightAvailable && (
        <p className="text-xs text-muted-foreground">
          Leverage highlighting is unavailable for this model.
        </p>
      )}
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
            <Chart model={model} />
          </>
        )
      )}
    </div>
  );
}

function LinearDiagnostics({ reference }: { reference: ResultReference }) {
  const query = useResultAnalysisQuery(reference, { kind: "diagnostics" });
  const value = query.value?.kind === "diagnostics" ? query.value.value : null;
  return (
    <div className="space-y-4">
      {query.error && <ResultReadError error={query.error} onRetry={() => void query.reload()} />}
      {query.loading && <p className="text-sm text-muted-foreground">Loading diagnostics…</p>}
      {value?.tests.map((test) => (
        <Section key={test.name} title={test.name} collapsible>
          {test.unavailable_reason ? (
            <p className="text-sm text-muted-foreground">Unavailable: {test.unavailable_reason}</p>
          ) : (
            <StructuredData value={test.value} />
          )}
        </Section>
      ))}
      {value && (
        <Section title="Leverage density" collapsible>
          {value.leverage_unavailable_reason ? (
            <p className="text-sm text-muted-foreground">
              Unavailable: {value.leverage_unavailable_reason}
            </p>
          ) : (
            <Chart
              model={{
                kind: "kde",
                points: [...value.leverage_density],
                xAxis: { label: "Leverage", valueType: "number" },
                yAxis: { label: "Density", valueType: "number" },
                xMin: 0,
              }}
            />
          )}
        </Section>
      )}
    </div>
  );
}

const CoefficientsContext = createContext<ReturnType<
  typeof useLinearRegressionCoefficients
> | null>(null);

function CoefficientsProvider({
  data,
  children,
}: {
  data: LinearRegressionReportData;
  children: ReactNode;
}) {
  const coefficients = useLinearRegressionCoefficients(data);
  return (
    <CoefficientsContext.Provider value={coefficients}>{children}</CoefficientsContext.Provider>
  );
}

export const LinearRegressionReport: FC<{
  data: LinearRegressionReportData;
  onValueChange?: (value: LinearRegressionReportData) => void;
}> = ({ data, onValueChange }) => {
  const contents = useLinearSummaryContents(data);
  useEffect(() => {
    onValueChange?.(contents.data);
  }, [contents.data, onValueChange]);
  const report = (
    <LinearReportContents
      key={`${contents.data.resultRef.executionSessionId}:${contents.data.resultRef.resultId}`}
      contents={contents}
    />
  );
  return contents.data.summary.coefficient_table ||
    contents.data.summary.coefficient_chart ||
    contents.data.summary.equation ? (
    <CoefficientsProvider
      key={`${contents.data.resultRef.executionSessionId}:${contents.data.resultRef.resultId}`}
      data={contents.data}
    >
      {report}
    </CoefficientsProvider>
  ) : (
    report
  );
};

function LinearCoefficientsSection({
  data,
  mode,
}: {
  data: LinearRegressionReportData;
  mode: "equation" | "table" | "chart";
}) {
  const value = useContext(CoefficientsContext);
  if (!value) throw new Error("Coefficient sections require their report context");
  const { page, coefficients, coefficientError } = value;
  if (mode === "equation")
    return coefficients.length === data.coefficients.rowCount && coefficients.length > 0 ? (
      <Suspense
        fallback={<div className="rounded-lg border border-border bg-card h-24 animate-pulse" />}
      >
        <LazyEquation endogName={data.endog_name} coefficients={coefficients} />
      </Suspense>
    ) : null;
  return (
    <>
      {page.error || coefficientError ? (
        <ResultReadError
          error={(page.error ?? coefficientError)!}
          onRetry={() => void page.reload()}
        />
      ) : null}
      {page.loading ? (
        <p className="text-sm text-muted-foreground">Loading coefficients…</p>
      ) : mode === "chart" ? (
        <Chart coefficients={coefficients} />
      ) : (
        <>
          <p className="mb-2 text-xs text-muted-foreground">
            {coefficients.filter((coefficient) => coefficient.is_significant).length}/
            {coefficients.length} significant on this page
          </p>
          <CoefficientTable coefficients={coefficients} />
        </>
      )}
      <PageToolbar page={page} />
    </>
  );
}

function LinearAnalysisSection({
  data,
  kind,
}: {
  data: LinearRegressionReportData;
  kind: "hypothesisTest" | "acfPacf" | "serialTests";
}) {
  const { t } = useTranslation();
  const query = useResultAnalysisQuery(
    data.resultRef,
    kind === "hypothesisTest"
      ? { kind: "hypothesis" }
      : kind === "acfPacf"
        ? { kind: "acfPacf" }
        : { kind: "serialTests" },
  );
  return (
    <Section title={t(`reportSections.${kind}`)} collapsible={false}>
      {query.error ? (
        <ResultReadError error={query.error} onRetry={() => void query.reload()} />
      ) : null}
      {query.loading ? (
        <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
      ) : null}
      {query.value?.kind === "hypothesis" && (
        <HypothesisTestResultView result={query.value.value} paramNames={data.paramNames} />
      )}
      {query.value?.kind === "acfPacf" && <AcfPacfResultView result={query.value.value} />}
      {query.value?.kind === "serialTests" && <SerialTestsResultView result={query.value.value} />}
    </Section>
  );
}

function LinearReportContents({
  contents,
}: {
  contents: ReturnType<typeof useLinearSummaryContents>;
}) {
  const { data } = contents;
  const { summary } = data;
  const { t } = useTranslation();
  return (
    <>
      <AddReportContents
        options={summary}
        paramNames={data.paramNames}
        busy={contents.busy}
        error={contents.error}
        onAdd={contents.add}
      />
      {!LINEAR_SUMMARY_CONTENTS.some(({ key }) => summary[key]) && (
        <p className="text-sm text-muted-foreground">{t("reportSummary.empty")}</p>
      )}
      <LinearReportSections data={data} />
    </>
  );
}

const LinearReportSections = memo(function LinearReportSections({
  data,
}: {
  data: LinearRegressionReportData;
}) {
  const { summary, presentation } = data;
  const { t } = useTranslation();
  return (
    <>
      {summary.equation &&
        data.coefficients.rowCount > 0 &&
        data.coefficients.rowCount <= LINEAR_COEFFICIENT_PAGE_SIZE && (
          <Section title={t("reportSections.equation")} collapsible={false}>
            <LinearCoefficientsSection data={data} mode="equation" />
          </Section>
        )}
      {summary.model_summary && (
        <Section title={t("reportSections.modelSummary")} collapsible={false}>
          <KeyValue items={presentation.summary.items} />
        </Section>
      )}
      {summary.anova && (
        <Section title={t("reportSections.anova")} collapsible={false}>
          <DataTable columns={presentation.anova.columns} rows={presentation.anova.rows} />
        </Section>
      )}
      {summary.coefficient_table && (
        <Section title={t("reportSections.coefficientTable")} collapsible={false}>
          <LinearCoefficientsSection data={data} mode="table" />
        </Section>
      )}
      {summary.coefficient_chart && (
        <Section title={t("reportSections.coefficientMagnitude")} collapsible={false}>
          <LinearCoefficientsSection data={data} mode="chart" />
        </Section>
      )}
      {summary.hypothesis_test && <LinearAnalysisSection data={data} kind="hypothesisTest" />}
      {summary.diagnostics && (
        <>
          <Section title={t("reportSections.diagnostics")} collapsible={false}>
            <StatCard {...presentation.conditionNumber.stat} />
          </Section>
          <Section title={t("reportSections.diagnosticTests")} collapsible>
            <LinearDiagnostics reference={data.resultRef} />
          </Section>
        </>
      )}
      {summary.residual_plot && (
        <Section title={t("reportSections.residualPlot")} collapsible>
          <LinearResidualPlot reference={data.resultRef} />
        </Section>
      )}
      {summary.observations && (
        <Section title={t("reportSections.observations")} collapsible>
          <LinearObservations data={data} />
        </Section>
      )}
      {summary.acf_pacf && <LinearAnalysisSection data={data} kind="acfPacf" />}
      {summary.serial_tests && <LinearAnalysisSection data={data} kind="serialTests" />}
    </>
  );
});
