import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { ResultJsonView } from "@/features/application/results/components/renderers/ResultRenderers";
import { ResultViewShell } from "@/features/application/results/components/ResultViewShell";
import type { ResultDescriptor } from "@/shared/types/domain/result";
import type { PresentationPayload } from "@/features/application/presentation";
import { ReportView } from "../info/ReportView";

export function ResultInspector({
  descriptor,
  payload,
}: {
  descriptor: ResultDescriptor;
  payload: Extract<PresentationPayload, { mode: "report" }>;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<"value" | "report">("value");
  const [openedReport, setOpenedReport] = useState(false);
  const [numericValue, setNumericValue] = useState(payload.data);
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
            validation={payload.validation}
            onValueChange={setNumericValue}
          />
        </div>
      )}
    </div>
  );
}
