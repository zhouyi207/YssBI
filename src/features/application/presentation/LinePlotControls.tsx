import { useCallback, useId, useState } from "react";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { LineChart } from "@/shared/charts/cartesian/LineChart";
import type { ChartModel } from "@/shared/charts/ChartModel";
import { ToolbarIconButton } from "@/shared/ui/ToolbarIconButton";

type LineChartModel = Extract<ChartModel, { kind: "line" }>;

export interface LinePlotControlsProps {
  model: LineChartModel;
}

export function LinePlotControls({ model }: LinePlotControlsProps) {
  const pointsSwitchId = useId();
  const [toolbarOpen, setToolbarOpen] = useState(false);
  const [pointsVisible, setPointsVisible] = useState(model.showPoints);
  const toggleToolbar = useCallback(() => setToolbarOpen((open) => !open), []);

  return (
    <div className="flex h-full min-h-0 w-full flex-col">
      <div className="flex items-center justify-end px-2 pt-1.5 pb-0">
        <ToolbarIconButton
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={toggleToolbar}
          tooltip="Toggle toolbar"
          aria-label="Toggle toolbar"
          className={
            toolbarOpen
              ? "bg-[var(--accent-color)]/10 text-[var(--accent-color)]"
              : "text-muted-foreground hover:text-foreground"
          }
        >
          <svg
            className="size-3.5"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </ToolbarIconButton>
      </div>

      {toolbarOpen ? (
        <div className="flex items-center gap-4 border-b border-border bg-muted/20 px-3 py-1.5">
          <div className="flex items-center gap-2">
            <Switch
              id={pointsSwitchId}
              size="sm"
              checked={pointsVisible}
              onCheckedChange={setPointsVisible}
            />
            <Label
              htmlFor={pointsSwitchId}
              className="cursor-pointer text-[11px] text-muted-foreground"
            >
              Scatter Points
            </Label>
          </div>
        </div>
      ) : null}

      <div className="min-h-0 flex-1">
        <LineChart
          data={model.points}
          xAxis={model.xAxis}
          yAxis={model.yAxis}
          showPoints={pointsVisible}
        />
      </div>
    </div>
  );
}
