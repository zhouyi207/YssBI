import {
  createContext,
  memo,
  useCallback,
  useContext,
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
  useLinearRegressionAnalysis,
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
import { ScatterChart } from "@/shared/charts/cartesian/ScatterChart";
import type { LinearRegressionReportData } from "@/shared/types/domain/resultReport";
import type { ResultReference } from "@/shared/types/domain/result";
import type { ReportSectionKind } from "@/shared/types/domain/uiPresentation";
import { useUiPage } from "@/features/application/presentation/useUiPage";
import { UiPageRenderer } from "@/components/ui-presentation/UiPageRenderer";
import { ResultPageLayoutControls } from "./ResultPageLayoutControls";
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

const MemoScatterChart = memo(ScatterChart);
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
  const points = useMemo(
    () => plot?.points.map((point) => ({ x: point.x, y: point.y })) ?? [],
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
            <MemoScatterChart
              data={points}
              xAxis={FITTED_AXIS}
              yAxis={RESIDUAL_AXIS}
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

export const LinearRegressionComponent: FC<{ data: LinearRegressionReportData }> = ({ data }) => (
  <CoefficientsProvider
    key={`${data.resultRef.executionSessionId}:${data.resultRef.resultId}`}
    data={data}
  >
    <LinearRegressionReport data={data} />
  </CoefficientsProvider>
);

function LinearCoefficientsSection({
  data,
  equation = false,
}: {
  data: LinearRegressionReportData;
  equation?: boolean;
}) {
  const value = useContext(CoefficientsContext);
  if (!value) throw new Error("Coefficient sections require their report context");
  const { page, coefficients, coefficientError } = value;
  if (equation)
    return coefficients.length === data.coefficients.rowCount && coefficients.length > 0 ? (
      <ReportSection title="Equation" icon="equation">
        <ReportLazyBoundary variant="formula">
          <LazyFormulaBlock endogName={data.endog_name} coefficients={coefficients} />
        </ReportLazyBoundary>
      </ReportSection>
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
      ) : (
        <CoefficientsBlock coefficients={coefficients} hasCategorical={false} />
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
  const { hypothesisSource, computeAcf, computeSerialTests } = useLinearRegressionAnalysis(data);
  if (kind === "hypothesisTest") return <HypothesisTestBlock source={hypothesisSource} />;
  if (kind === "acfPacf")
    return <ACFPACFBlock observationCount={data.observations.rowCount} compute={computeAcf} />;
  return (
    <SerialTestsBlock observationCount={data.observations.rowCount} compute={computeSerialTests} />
  );
}

const LinearReportSection = memo(function LinearReportSection({
  data,
  kind,
}: {
  data: LinearRegressionReportData;
  kind: ReportSectionKind;
}) {
  switch (kind) {
    case "equation":
      return data.coefficients.rowCount > 0 &&
        data.coefficients.rowCount <= LINEAR_COEFFICIENT_PAGE_SIZE ? (
        <LinearCoefficientsSection data={data} equation />
      ) : null;
    case "coefficientTable":
      return <LinearCoefficientsSection data={data} />;
    case "modelSummary":
      return (
        <ReportSection title="Model Summary" icon="modelSummary">
          <ModelSummaryGrid info={data.model_basic_info} />
        </ReportSection>
      );
    case "anova":
      return (
        <ReportSection title="ANOVA" icon="anova">
          <AnovaTable info={data.model_basic_info} />
        </ReportSection>
      );
    case "diagnostics":
      return (
        <ReportSection title="Diagnostics" icon="test">
          <p className="text-sm text-muted-foreground">
            Condition number: {data.diagnostic_info.cond_no}
          </p>
        </ReportSection>
      );
    case "residualPlot":
      return (
        <ExpandableReportSection title="Residuals vs Fitted">
          <LinearResidualPlot reference={data.resultRef} />
        </ExpandableReportSection>
      );
    case "observations":
      return (
        <ExpandableReportSection title="Fitted values and residuals">
          <LinearObservations data={data} />
        </ExpandableReportSection>
      );
    case "hypothesisTest":
    case "acfPacf":
    case "serialTests":
      return <LinearAnalysisSection data={data} kind={kind} />;
  }
});

function LinearRegressionReport({ data }: { data: LinearRegressionReportData }) {
  const { t } = useTranslation();
  const presentation = useUiPage(data.resultRef);
  const info = data.model_basic_info;
  const renderSection = useCallback(
    (kind: ReportSectionKind) => <LinearReportSection data={data} kind={kind} />,
    [data],
  );
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
            renderReportSection={renderSection}
            disabled={presentation.busy}
            onActivate={(id) => void presentation.activate(id)}
          />
        </>
      )}
    </ReportLayout>
  );
}
