import { useMemo } from "react";
import type { ResultReference } from "@/shared/types/domain/result";
import {
  parseStructuredReportRows,
  type ReportTableSection,
} from "@/shared/types/domain/structuredReportDisplay";
import { usePagedResultRows } from "./usePagedResultRows";

export function useStructuredReportTable(reference: ResultReference, section: ReportTableSection) {
  const page = usePagedResultRows(
    reference,
    section.value.rowCount,
    100,
    undefined,
    section.value.part,
  );
  const content = useMemo(
    () => parseStructuredReportRows(section, page.rows),
    [section, page.rows],
  );
  return { page, content };
}
