import type { IJsonModel } from "flexlayout-react";
import { logsLayoutRuntime } from "./logsRuntime";

export interface LogsLayoutRead {
  subscribe(listener: () => void): () => void;
  getLatestSnapshot(): IJsonModel;
}

export const logsLayoutRead: LogsLayoutRead = {
  subscribe: logsLayoutRuntime.subscribe,
  getLatestSnapshot: logsLayoutRuntime.getLatestSnapshot,
};
