import type { SerializedDockview } from "dockview-react";
import { logsDockviewRuntime } from "./logsRuntime";

export interface LogsDockviewRead {
  subscribe(listener: () => void): () => void;
  getLatestSnapshot(): SerializedDockview;
}

export const logsDockviewRead: LogsDockviewRead = {
  subscribe: logsDockviewRuntime.subscribe,
  getLatestSnapshot: logsDockviewRuntime.getLatestSnapshot,
};
