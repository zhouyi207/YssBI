import { useEffect, useState } from "react";
import { request } from "@/sdk";
import type { BayesColumnMetaDTO, BayesDatasetSelectionDTO } from "@/shared/types/bayes";
import type { ErrorReference } from "@/features/application/errorReference";
export interface BayesDatasetOption {
  readonly sourceType: BayesDatasetSelectionDTO["sourceType"];
  readonly sourceId: string;
  readonly columns: readonly BayesColumnMetaDTO[];
  readonly displayName: string;
}
export function bayesColumnDType(value: string): BayesColumnMetaDTO["dtype"] {
  const type = value.toLowerCase();
  if (type.includes("int")) return "integer";
  if (/float|double|decimal|numeric/.test(type)) return "number";
  if (type.includes("bool")) return "boolean";
  if (/date|time/.test(type)) return "date";
  if (/string|text|char/.test(type)) return "string";
  return "unknown";
}
export function useBayesDatasets() {
  const [datasets, setDatasets] = useState<BayesDatasetOption[]>([]);
  const [loading, setLoading] = useState(true);
  const [issue, setIssue] = useState<ErrorReference | null>(null);
  useEffect(() => {
    let disposed = false;
    void request<{
      datasets: {
        id: string;
        name: string;
        columns: { name: string; type: string; nullable: boolean }[];
      }[];
    }>("data.list")
      .then((value) => {
        if (!disposed)
          setDatasets(
            value.datasets.map((dataset) => ({
              sourceId: dataset.id,
              sourceType: "table",
              displayName: dataset.name,
              columns: dataset.columns.map((column) => ({
                name: column.name,
                dtype: bayesColumnDType(column.type),
                nullable: column.nullable,
              })),
            })),
          );
      })
      .catch(() => {
        if (!disposed) setIssue({ code: "bayes_dataset_metadata_read_failed", incidentId: null });
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, []);
  return { datasets, loading, issue };
}
