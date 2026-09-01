import { useEffect } from "react";
import { useChartRead } from "@/features/core/chart/read";
import type { EditorPanelScope } from "@/modules/workbench/public";
import { loadChartDocumentForView } from "@/features/application/chart/chartViewActions";
import { ChartPreview } from "./ChartPreview";

export type ChartEditorProps = EditorPanelScope<"chart">;

export function ChartEditor(props: ChartEditorProps) {
  const chartPath = props.resourceRef;
  const document = useChartRead((snapshot) => snapshot.documents[chartPath] ?? null);
  const hasDocument = useChartRead((snapshot) => Boolean(snapshot.documents[chartPath]));

  useEffect(() => {
    if (hasDocument) return;
    void loadChartDocumentForView(chartPath);
  }, [chartPath, hasDocument]);

  return (
    <div
      className="flex h-full w-full min-h-0 flex-col"
      data-chart-editor
      data-panel-instance-id={props.panelInstanceId}
      data-group-id={props.groupId}
    >
      <ChartPreview chartPath={chartPath} document={document} />
    </div>
  );
}
