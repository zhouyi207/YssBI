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
import { ResultReportPage } from "./ResultReportPage";

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
  value,
}: {
  reference: ResultReference;
  value: unknown;
}) {
  const { t } = useTranslation();
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
  return (
    <ResultReportPage
      reference={reference}
      data={{}}
      bindings={{ result: { type: "structured", value, renderReference } }}
    />
  );
}
