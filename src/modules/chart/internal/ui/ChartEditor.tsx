import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { useChartRead } from "@/features/core/chart/read";
import type { EditorPanelScope } from "@/modules/workbench/public";
import { useChartDocumentLoad } from "@/features/application/chart/useChartDocumentLoad";
import { ChartPreview } from "./ChartPreview";

export type ChartEditorProps = EditorPanelScope<"chart">;

export function ChartEditor(props: ChartEditorProps) {
  const { t } = useTranslation();
  const chartPath = props.resourceRef;
  const document = useChartRead((snapshot) => snapshot.documents[chartPath] ?? null);
  const { failed, retry } = useChartDocumentLoad(chartPath, Boolean(document));

  return (
    <div
      className="flex h-full w-full min-h-0 flex-col"
      data-chart-editor
      data-panel-instance-id={props.panelInstanceId}
      data-group-id={props.groupId}
    >
      {document ? (
        <ChartPreview chartPath={chartPath} document={document} />
      ) : (
        <div className="flex h-full flex-col items-center justify-center gap-3 p-5 text-sm">
          <p role={failed ? "alert" : "status"} className="text-muted-foreground">
            {t(failed ? "detail.loadFailed" : "common.loading")}
          </p>
          {failed && (
            <Button size="sm" variant="outline" onClick={retry}>
              {t("common.retry")}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
