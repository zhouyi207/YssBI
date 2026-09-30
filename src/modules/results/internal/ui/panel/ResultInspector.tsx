import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { ResultJsonView } from "@/features/application/results/components/renderers/ResultRenderers";
import { ResultViewShell } from "@/features/application/results/components/ResultViewShell";
import type { ResultDescriptor, ResultReportKind } from "@/shared/types/domain/result";
import { ReportView } from "../info/ReportView";

export function ResultInspector({
  descriptor,
  report,
  data,
}: {
  descriptor: ResultDescriptor;
  report: ResultReportKind;
  data: unknown;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<"value" | "report">("value");
  const [openedReport, setOpenedReport] = useState(false);
  const [numericValue, setNumericValue] = useState(data);
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div
        className="flex h-(--panel-toolbar-height) shrink-0 items-center justify-end gap-1 border-b border-border/20 px-2"
        role="group"
        aria-label={t("sourceInspector.viewMode")}
      >
        <Button
          size="xs"
          variant={mode === "value" ? "secondary" : "ghost"}
          aria-pressed={mode === "value"}
          onClick={() => setMode("value")}
        >
          {t("sourceInspector.numericView")}
        </Button>
        <Button
          size="xs"
          variant={mode === "report" ? "secondary" : "ghost"}
          aria-pressed={mode === "report"}
          onClick={() => {
            setOpenedReport(true);
            setMode("report");
          }}
        >
          {t("sourceInspector.reportView")}
        </Button>
      </div>
      {mode === "value" && (
        <ResultViewShell title={descriptor.title}>
          <ResultJsonView value={numericValue} />
        </ResultViewShell>
      )}
      {openedReport && (
        <div className={mode === "report" ? "flex min-h-0 flex-1 flex-col" : "hidden"}>
          <ReportView
            descriptor={descriptor}
            report={report}
            data={data}
            onValueChange={setNumericValue}
          />
        </div>
      )}
    </div>
  );
}
