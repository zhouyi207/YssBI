import { createContext, useContext, useMemo, useState, type FC, type ReactNode } from "react";
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
import { Section } from "@/components/ui-presentation/Section";
import { Chart } from "@/components/ui-presentation/Chart";
import type { ChartModel } from "@/shared/charts/ChartModel";
import {
  LINEAR_SUMMARY_CONTENTS,
  type LinearRegressionReportData,
} from "@/shared/types/domain/resultReport";
import type { ResultReference } from "@/shared/types/domain/result";
import { useUiPage } from "@/features/application/presentation/useUiPage";
import { UiPageRenderer, type UiResultBindings } from "@/components/ui-presentation/UiPageRenderer";
import { ResultPageLayoutControls } from "./ResultPageLayoutControls";
import { ReportLayout, ReportLazyBoundary, RSquaredBadge, LazyEquation } from "./shared";
import { CoefficientTable } from "@/components/ui-presentation/CoefficientTable";
import { HypothesisTestResultView } from "./shared/HypothesisTestBlock";
import { AcfPacfResultView } from "./shared/ACFPACFBlock";
import { SerialTestsResultView } from "./shared/SerialTestsBlock";
import { useLinearSummaryContents } from "@/features/application/results/useLinearSummaryContents";
import { AddReportContents } from "./AddReportContents";

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
  const columns = useMemo(() => [...page.columns], [page.columns]);
  const rows = useMemo(() => page.rows.map((row) => [...row]), [page.rows]);
  return (
    <div className="space-y-3">
      {page.error ? (
        <ResultReadError error={page.error} onRetry={() => void page.reload()} />
      ) : null}
      <ReadOnlyDataGrid
        columns={columns}
        rows={rows}
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
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [xRange, setXRange] = useState<[number, number] | undefined>();
  const query = useResultAnalysisQuery(reference, {
    kind: "residualPlot",
    maxPoints: 2000,
    xRange,
  });
  const plot = query.value?.kind === "residualPlot" ? query.value.value : null;
  const model = useMemo<ChartModel>(
    () => ({
      kind: "scatter",
      points: plot?.points.map((point) => ({ x: point.x, y: point.y })) ?? [],
      xAxis: FITTED_AXIS,
      yAxis: RESIDUAL_AXIS,
      symmetricY: true,
      zeroLine: true,
    }),
    [plot?.points],
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
            <Chart model={model} />
          </>
        )
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

export const LinearRegressionComponent: FC<{ data: LinearRegressionReportData }> = ({ data }) => {
  const contents = useLinearSummaryContents(data);
  const report = (
    <LinearRegressionReport
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
      <ReportLazyBoundary variant="formula">
        <LazyEquation endogName={data.endog_name} coefficients={coefficients} />
      </ReportLazyBoundary>
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
          <CoefficientTable coefficients={coefficients} hasCategorical={false} />
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
    <Section title={t(`reportLayout.sections.${kind}`)} collapsible={false}>
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

function LinearRegressionReport({
  contents,
}: {
  contents: ReturnType<typeof useLinearSummaryContents>;
}) {
  const { data } = contents;
  const { t } = useTranslation();
  const presentation = useUiPage(data.resultRef);
  const summary = data.presentation.summary.items;
  const results = useMemo<UiResultBindings>(
    () => ({
      equation: {
        type: "equation",
        available:
          data.summary.equation &&
          data.coefficients.rowCount > 0 &&
          data.coefficients.rowCount <= LINEAR_COEFFICIENT_PAGE_SIZE,
        content: <LinearCoefficientsSection data={data} mode="equation" />,
      },
      coefficients: {
        type: "coefficientTable",
        available: data.summary.coefficient_table,
        content: <LinearCoefficientsSection data={data} mode="table" />,
      },
      coefficientMagnitude: {
        type: "chart",
        available: data.summary.coefficient_chart,
        content: <LinearCoefficientsSection data={data} mode="chart" />,
      },
      residualPlot: {
        type: "chart",
        available: data.summary.residual_plot,
        content: <LinearResidualPlot reference={data.resultRef} />,
      },
      observations: {
        type: "table",
        available: data.summary.observations,
        content: <LinearObservations data={data} />,
      },
      hypothesis: {
        type: "analysis",
        available: data.summary.hypothesis_test,
        content: <LinearAnalysisSection data={data} kind="hypothesisTest" />,
      },
      acfPacf: {
        type: "analysis",
        available: data.summary.acf_pacf,
        content: <LinearAnalysisSection data={data} kind="acfPacf" />,
      },
      serialTests: {
        type: "analysis",
        available: data.summary.serial_tests,
        content: <LinearAnalysisSection data={data} kind="serialTests" />,
      },
    }),
    [data],
  );
  return (
    <ReportLayout
      title={data.title}
      badges={
        <>
          <RSquaredBadge value={summary.find((item) => item.id === "rSquared")?.value} />
          <span className="text-xs text-muted-foreground">
            {summary.find((item) => item.id === "method")?.value} &middot; n=
            {data.observations.rowCount}
          </span>
        </>
      }
    >
      <AddReportContents
        options={data.summary}
        paramNames={data.paramNames}
        busy={contents.busy}
        error={contents.error}
        onAdd={contents.add}
      />
      {!LINEAR_SUMMARY_CONTENTS.some(({ key }) => data.summary[key]) && (
        <p className="text-sm text-muted-foreground">{t("reportSummary.empty")}</p>
      )}
      {presentation.error && (
        <ResultReadError error={presentation.error} onRetry={() => void presentation.reload()} />
      )}
      {!presentation.page && !presentation.error && (
        <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
      )}
      {presentation.page && (
        <>
          <ResultPageLayoutControls
            spec={presentation.page.spec}
            busy={presentation.busy}
            onAction={presentation.act}
          />
          <UiPageRenderer
            spec={presentation.page.spec}
            data={data.presentation}
            results={results}
            disabled={presentation.busy}
            onActivate={(id) => void presentation.activate(id)}
          />
        </>
      )}
    </ReportLayout>
  );
}
