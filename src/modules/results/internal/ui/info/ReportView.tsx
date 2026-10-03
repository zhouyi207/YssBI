import { useEffect, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { ResultDescriptor } from "@/features/application/results/types";
import type { ReportValidationResult } from "@/shared/types/report/reportValidation";
import { reportInvalidReport } from "@/features/application/observability/reportViewIssue";
import { LinearResultBindings } from "./LinearResultBindings";
import { StructuredResult } from "./StructuredResult";
import type { LinearRegressionReportData } from "@/shared/types/domain/resultReport";

interface ReportViewProps {
  descriptor: ResultDescriptor;
  validation: ReportValidationResult;
  onValueChange?: (value: LinearRegressionReportData) => void;
}

export function ReportView({ descriptor, validation, onValueChange }: ReportViewProps) {
  useEffect(() => {
    if (!validation.ok) {
      reportInvalidReport(validation.diagnostic);
    }
  }, [validation]);

  let content: ReactNode;
  if (!validation.ok) {
    const label =
      validation.diagnostic.presentation.report === "linearRegressionSummary"
        ? "linear regression report"
        : "report";
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
      validation.value.kind === "linearRegressionSummary" ? (
        <LinearResultBindings data={validation.value.data} onValueChange={onValueChange} />
      ) : (
        <StructuredResult reference={descriptor} report={validation.value} />
      );
  }

  return (
    <ScrollArea className="min-h-0 flex-1" orientation="vertical">
      {content}
    </ScrollArea>
  );
}
