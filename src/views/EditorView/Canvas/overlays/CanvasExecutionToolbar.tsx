import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useExecutionPlayback } from "@/features/core/execution/useExecutionPlayback";
import { useExecutionRead } from '@/features/core/execution/read';
import { graphHasClearableArtifacts } from "@/features/core/execution/graphRunArtifacts";
import {
  VscClearAll,
  VscDebugPause,
  VscDebugStop,
  VscDebugRestart,
  VscPlay,
  VscRunAll,
} from "react-icons/vsc";
import { useTranslation } from "react-i18next";

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
  onExecute,
  onCancelExecution,
  onClearArtifacts,
}: {
  graphPath: string;
  onExecute: () => void;
  onCancelExecution: () => void;
  onClearArtifacts: () => void;
}) {
  const { t } = useTranslation();
  const { stop: stopReplay, togglePlayPause, isPlaying, isPaused, hasRecording, graphDirty } =
    useExecutionPlayback(graphPath);
  const graphState = useExecutionRead((snapshot) => snapshot.graphs[graphPath]);
  const graphStatus = graphState?.status ?? "idle";

  const playbackActive = isPlaying || isPaused;
  const isLiveRunning = graphStatus === "running";
  const canReplay = hasRecording && !graphDirty && !isLiveRunning;
  const canClear =
    !isLiveRunning
    && !playbackActive
    && graphHasClearableArtifacts(graphState);

  return (
    <div className="absolute top-3 right-3 z-40 flex items-center gap-1 bg-[var(--panel-bg)]/80 backdrop-blur-sm border border-[var(--border-color)] rounded-md p-0.5 shadow-lg">
      {!playbackActive ? (
        <CanvasToolbarButton
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => canReplay && togglePlayPause()}
          disabled={!canReplay}
          className={
            canReplay
              ? "text-blue-400 hover:text-blue-300"
              : "text-[var(--text-secondary)] opacity-40 cursor-not-allowed"
          }
          tooltip={
            graphDirty
              ? t("canvas.replayDisabledDirty")
              : !hasRecording
                ? t("canvas.replayNoRecording")
                : t("canvas.replayExecution")
          }
        >
          <VscDebugRestart size={14} />
        </CanvasToolbarButton>
      ) : (
        <div className="flex items-center">
          <CanvasToolbarButton
            type="button"
            variant="ghost"
            size="sm"
            onClick={togglePlayPause}
            className={isPlaying ? "text-amber-400" : "text-blue-400"}
            tooltip={isPlaying ? t("canvas.pauseReplay") : t("canvas.resumeReplay")}
          >
            {isPlaying ? <VscDebugPause size={14} /> : <VscPlay size={14} />}
          </CanvasToolbarButton>
          <CanvasToolbarButton
            type="button"
            variant="ghost"
            size="sm"
            onClick={stopReplay}
            className="text-red-400"
            tooltip={t("canvas.stopReplay")}
          >
            <VscDebugStop size={14} />
          </CanvasToolbarButton>
        </div>
      )}

      <div className="w-px h-5 bg-[var(--border-color)]" />

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
        disabled={isLiveRunning}
        className={
          isLiveRunning
            ? "text-green-400 opacity-60 cursor-not-allowed"
            : "text-green-400 hover:text-green-300"
        }
        tooltip={isLiveRunning ? t("canvas.executing") : t("canvas.executeCurrentEvent")}
      >
        <VscRunAll size={14} />
      </CanvasToolbarButton>
    </div>
  );
}
