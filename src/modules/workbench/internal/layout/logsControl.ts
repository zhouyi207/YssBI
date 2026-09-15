import type { IJsonModel } from "flexlayout-react";
import { logsLayoutRuntime } from "./logsRuntime";

export interface LogsLayoutControl {
  beginRestore(): number;
  stageRestore(epoch: number, layout: IJsonModel): "staged" | "applied" | "stale";
  captureBoundSnapshot(): void;
  resetToDefault(): void;
}

export const logsLayoutControl: LogsLayoutControl = {
  beginRestore: logsLayoutRuntime.beginRestore,
  stageRestore: logsLayoutRuntime.stageRestore,
  captureBoundSnapshot: logsLayoutRuntime.captureBoundSnapshot,
  resetToDefault: logsLayoutRuntime.resetToDefault,
};
