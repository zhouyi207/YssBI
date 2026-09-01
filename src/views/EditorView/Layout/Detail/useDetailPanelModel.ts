import { useMemo } from "react";
import { useEditorSessionResources } from "@/features/application/editor";
import { useEditorUi } from "@/features/core/editor/ui";
import { useLogStore } from "@/features/application/log";
import { useWorksheetRead } from "@/features/core/worksheet/read";
import type { WorksheetDocument } from "@/shared/types/domain/worksheet";
import { resolveDetailPanelModel } from "./resolveDetailPanelModel";
import type { DetailPanelModel } from "./resolveDetailPanelModel";

export function useDetailPanelModel(): {
  model: DetailPanelModel;
  worksheetPath: string | null;
  worksheetName: string | null;
  worksheetDocument: WorksheetDocument | null;
} {
  const { variables, events, functions, dataframes } = useEditorSessionResources();
  const target = useEditorUi((snapshot) => snapshot.detailFocus);
  const selectedLog = useLogStore((s) => s.selectedLog);

  const worksheetPath = target?.kind === "worksheet" ? target.worksheetPath : null;

  const worksheetDocument = useWorksheetRead((snapshot) =>
    worksheetPath
      ? snapshot.documents[worksheetPath]
        ? (structuredClone(snapshot.documents[worksheetPath]) as WorksheetDocument)
        : null
      : null,
  );
  const worksheetName = useWorksheetRead((snapshot) =>
    worksheetPath
      ? (snapshot.index.find((worksheet) => worksheet.worksheetPath === worksheetPath)?.name ??
        null)
      : null,
  );

  const model = useMemo(
    () =>
      resolveDetailPanelModel({
        target,
        selectedLog,
        variables,
        events,
        functions,
        dataframes,
        worksheetDocument,
      }),
    [target, selectedLog, variables, events, functions, dataframes, worksheetDocument],
  );

  return { model, worksheetPath, worksheetName, worksheetDocument };
}
