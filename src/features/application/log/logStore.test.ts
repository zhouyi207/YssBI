import { beforeEach, describe, expect, it } from "vitest";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { applyLogFilter, type LogLogFilter, useLogStore } from "./logStore";

function record(
  domain: LogRecordDto["domain"],
  sequence: number,
  level: LogRecordDto["level"] = "info",
  message = `${domain} message`,
): LogRecordDto {
  return {
    streamId: "stream-1",
    sequence,
    timestamp: "2026-08-16T10:11:12.000Z",
    level,
    origin: "rust",
    domain,
    target: `${domain}.target`,
    message,
    fields: {},
  };
}

const allLevels = new Set<LogRecordDto["level"]>(["trace", "debug", "info", "warn", "error"]);

describe("diagnostic log domain filtering", () => {
  beforeEach(() => {
    useLogStore.setState({
      filter: { levels: new Set(allLevels), searchText: "" },
      selectedLog: null,
      autoScroll: true,
    });
  });

  it("applies shared level and search filters independently per domain", () => {
    const logs = [
      record("graph", 1, "info", "graph ready"),
      record("application", 2, "info", "application ready"),
      record("graph", 3, "warn", "graph ready with warning"),
      record("graph", 4, "info", "graph waiting"),
    ];
    const filter: LogLogFilter = {
      levels: new Set(["info"]),
      searchText: "ready",
    };

    expect(applyLogFilter(logs, filter, "graph").map((log) => log.domain)).toEqual(["graph"]);
    expect(applyLogFilter(logs, filter, "all").map((log) => log.domain)).toEqual([
      "graph",
      "application",
    ]);
  });

  it("preserves autoScroll when the main Logs consumer is reopened", () => {
    useLogStore.getState().setAutoScroll(false);

    useLogStore.getState().setSearchText("ready");
    const reopenedState = useLogStore.getState();

    expect(reopenedState.autoScroll).toBe(false);
    expect(reopenedState.filter.searchText).toBe("ready");
  });

  it("shares filtered records between consumers and invalidates them for changed inputs", () => {
    const logs = [record("graph", 1, "info", "ready"), record("execution", 2, "warn", "ready")];
    const filter: LogLogFilter = { levels: allLevels, searchText: "ready" };
    const graph = applyLogFilter(logs, filter, "graph");
    expect(applyLogFilter(logs, filter, "graph")).toBe(graph);
    expect(applyLogFilter(logs, filter, "all")).toEqual(logs);
    expect(applyLogFilter(logs, filter, "graph")).toBe(graph);

    const warnings = { ...filter, levels: new Set(["warn"] as const) };
    expect(applyLogFilter(logs, warnings, "graph")).toEqual([]);
    expect(applyLogFilter(logs, warnings, "all")).toEqual([logs[1]]);
    expect(applyLogFilter(logs, { ...filter, searchText: "waiting" }, "all")).toEqual([]);
    const appended = [...logs, record("graph", 3, "info", "ready")];
    expect(applyLogFilter(appended, filter, "graph")).toEqual([logs[0], appended[2]]);
    expect(applyLogFilter(logs, filter, "graph")).toBe(graph);
  });
});
