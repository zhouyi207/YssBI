import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useExecutionRead } from "@/features/core/execution/read";
import { graphHasClearableArtifacts } from "@/features/core/execution/graphRunArtifacts";
import { VscClearAll, VscDebugStop, VscRunAll } from "react-icons/vsc";
import { useTranslation } from "react-i18next";
import { useGraphEditingUi } from "@/features/core/graphEditing/ui";

function CanvasToolbarButton({
  tooltip,
  children,
  ...props
}: React.ComponentProps<typeof Button> & { tooltip: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button {...props}>{children}</Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{tooltip}</TooltipContent>
    </Tooltip>
  );
}

export function CanvasExecutionToolbar({
  graphPath,
  canExecute,
  executeUnavailableReason,
  onExecute,
  onCancelExecution,
  onClearArtifacts,
}: {
  graphPath: string;
  canExecute: boolean;
  executeUnavailableReason: "functionGraph" | "blockingProblems" | null;
  onExecute: () => void;
  onCancelExecution: () => void;
  onClearArtifacts: () => void;
}) {
  const { t } = useTranslation();
  const { saving } = useGraphEditingUi(graphPath);
  const graphState = useExecutionRead((snapshot) => snapshot.graphs[graphPath]);
  const graphStatus = graphState?.status ?? "idle";

  const isLiveRunning = graphStatus === "running";
  const canClear = !isLiveRunning && graphHasClearableArtifacts(graphState);
  const canRun = canExecute && !saving && !isLiveRunning;

  return (
    <div className="absolute top-3 right-3 z-40 flex items-center gap-1 bg-[var(--panel-bg)]/80 backdrop-blur-sm border border-[var(--border-color)] rounded-md p-0.5 shadow-lg">
      <CanvasToolbarButton
        type="button"
        variant="ghost"
        size="sm"
        onClick={onClearArtifacts}
        disabled={!canClear}
        className={
          canClear
            ? "text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
            : "text-[var(--text-secondary)] opacity-40 cursor-not-allowed"
        }
        tooltip={
          canClear
            ? t("canvas.clearExecutionArtifacts")
            : t("canvas.clearExecutionArtifactsDisabled")
        }
      >
        <VscClearAll size={14} />
      </CanvasToolbarButton>

      {isLiveRunning && graphState?.runId && (
        <CanvasToolbarButton
          type="button"
          variant="ghost"
          size="sm"
          onClick={onCancelExecution}
          className="text-red-400 hover:text-red-300"
          tooltip={t("canvas.cancelExecution")}
        >
          <VscDebugStop size={14} />
        </CanvasToolbarButton>
      )}

      <CanvasToolbarButton
        type="button"
        variant="ghost"
        size="sm"
        onClick={onExecute}
        disabled={!canRun}
        className={
          !canRun
            ? "text-green-400 opacity-60 cursor-not-allowed"
            : "text-green-400 hover:text-green-300"
        }
        tooltip={
          isLiveRunning
            ? t("canvas.executing")
            : saving
              ? t("canvas.savingGraph")
              : executeUnavailableReason === "functionGraph"
                ? t("canvas.functionRunUnavailable")
                : executeUnavailableReason === "blockingProblems"
                  ? t("canvas.problemsBlockExecution")
                  : t("canvas.executeCurrentGraph")
        }
      >
        <VscRunAll size={14} />
      </CanvasToolbarButton>
    </div>
  );
}
