import type { ParsedReportPayload } from "@/shared/types/report/parseReportPayload";
import { StructuredReportTable } from "./StructuredReportTable";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Section } from "@/components/ui-presentation/Section";
import {
  StructuredData,
  type StructuredReferenceRenderer,
} from "@/components/ui-presentation/StructuredData";
import { usePagedResultRows } from "@/features/application/results/usePagedResultRows";
import { ResultReadError } from "@/features/application/results/components/ResultReadError";
import type { ResultReference } from "@/shared/types/domain/result";
import {
  isStructuredTableReference,
  type ResultTableReference,
  type StructuredResultPart,
} from "@/shared/types/domain/resultReport";

function StructuredArray({
  reference,
  table,
  renderReference,
}: {
  reference: ResultReference;
  table: ResultTableReference<StructuredResultPart>;
  renderReference: StructuredReferenceRenderer;
}) {
  const { t } = useTranslation();
  const page = usePagedResultRows(reference, table.rowCount, 100, undefined, table.part);
  const values = useMemo(() => page.rows.map((row) => row[0]), [page.rows]);
  if (page.error) return <ResultReadError error={page.error} onRetry={() => void page.reload()} />;
  return (
    <div className="min-w-0 space-y-3">
      {page.loading ? (
        <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
      ) : (
        <StructuredData value={values} renderReference={renderReference} />
      )}
      <div className="flex items-center justify-end gap-2 text-xs text-muted-foreground">
        <span>
          {page.actualCount ? page.offset + 1 : 0}–{page.offset + page.actualCount} /{" "}
          {page.totalCount ?? table.rowCount}
        </span>
        <Button
          size="xs"
          variant="outline"
          disabled={page.loading || page.pageIndex === 0}
          onClick={page.goToPreviousPage}
        >
          {t("sourceInspector.previous")}
        </Button>
        <Button
          size="xs"
          variant="outline"
          disabled={page.loading || !page.hasMore}
          onClick={page.goToNextPage}
        >
          {t("sourceInspector.next")}
        </Button>
      </div>
    </div>
  );
}

export function StructuredResult({
  reference,
  report,
}: {
  reference: ResultReference;
  report: Extract<ParsedReportPayload, { kind: "structured" }>;
}) {
  const { t } = useTranslation();
  const renderReference = useMemo<StructuredReferenceRenderer>(() => {
    const renderReference: StructuredReferenceRenderer = (candidate) => {
      if (!isStructuredTableReference(candidate)) return undefined;
      return (
        <Section
          key={candidate.part}
          title={t("sourceInspector.inspectRows", { count: candidate.rowCount })}
          collapsible
        >
          <StructuredArray
            reference={reference}
            table={candidate}
            renderReference={renderReference}
          />
        </Section>
      );
    };
    return renderReference;
  }, [reference, t]);
  const sections = Object.entries(report.sections);
  const raw = <StructuredData value={report.data} renderReference={renderReference} />;
  return (
    <>
      {sections.map(([id, section]) => (
        <Section key={id} title={section.title} collapsible>
          {section.kind === "equation" ? (
            <pre className="overflow-x-auto whitespace-pre-wrap rounded-md bg-muted/30 p-4 font-mono text-sm">
              {section.value}
            </pre>
          ) : (
            <StructuredReportTable reference={reference} section={section} />
          )}
        </Section>
      ))}
      {sections.length > 0 ? (
        <Section title={t("reportSections.structuredResult")} collapsible>
          {raw}
        </Section>
      ) : (
        raw
      )}
    </>
  );
}
