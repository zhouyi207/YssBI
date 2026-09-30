import { useEffect, useMemo, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { ReportKind, ResultDescriptor } from "@/features/application/results/types";
import { validateReportPayload } from "@/shared/types/report/reportValidation";
import { reportViewIssue } from "@/features/application/observability/reportViewIssue";
import { LinearResultBindings } from "./LinearResultBindings";
import { ResultReportPage } from "./ResultReportPage";
import type { LinearRegressionReportData } from "@/shared/types/domain/resultReport";

interface ReportViewProps {
  descriptor: ResultDescriptor;
  report: ReportKind;
  data: unknown;
  onValueChange?: (value: LinearRegressionReportData) => void;
}

export function ReportView({ descriptor, report, data, onValueChange }: ReportViewProps) {
  const validation = useMemo(
    () => validateReportPayload(descriptor, report, data),
    [descriptor, report, data],
  );

  useEffect(() => {
    if (!validation.ok) {
      reportViewIssue("data", JSON.stringify(validation.diagnostic), "ReportValidation");
    }
  }, [validation]);

  let content: ReactNode;
  if (!validation.ok) {
    const label = report === "linearRegressionSummary" ? "linear regression report" : "report";
    content = (
      <Alert variant="destructive" className="m-4 w-auto">
        <AlertDescription className="text-destructive">
          Unable to render {label}: {validation.diagnostic.fieldPath} {validation.diagnostic.reason}
          .
        </AlertDescription>
      </Alert>
    );
  } else {
    content =
      report === "linearRegressionSummary" ? (
        <LinearResultBindings
          data={validation.value as LinearRegressionReportData}
          onValueChange={onValueChange}
        />
      ) : (
        <ResultReportPage
          reference={descriptor}
          data={{}}
          bindings={{ result: { type: "structured", value: validation.value } }}
        />
      );
  }

  return (
    <ScrollArea className="min-h-0 flex-1" orientation="vertical">
      {content}
    </ScrollArea>
  );
}
