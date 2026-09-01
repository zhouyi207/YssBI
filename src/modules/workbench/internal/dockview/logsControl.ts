import type { SerializedDockview } from "dockview-react";
import { logsDockviewRuntime } from "./logsRuntime";

export interface LogsDockviewControl {
  beginRestore(): number;
  stageRestore(epoch: number, layout: SerializedDockview): "staged" | "applied" | "stale";
  captureBoundSnapshot(): void;
  resetToDefault(): void;
}

export const logsDockviewControl: LogsDockviewControl = {
  beginRestore: logsDockviewRuntime.beginRestore,
  stageRestore: logsDockviewRuntime.stageRestore,
  captureBoundSnapshot: logsDockviewRuntime.captureBoundSnapshot,
  resetToDefault: logsDockviewRuntime.resetToDefault,
};
