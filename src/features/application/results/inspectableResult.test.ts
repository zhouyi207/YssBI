import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { PinResultEntry } from "@/shared/types/domain/result";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import {
  createResultQueryCoordinator,
  type ResultQueryDependencies,
  type ResultQueryReadCapability,
  type ResultQueryPublication,
} from "./resultQueryCoordinator";
import type { ResultDescriptor, ResultPage, ResultValue } from "./types";
import {
  outputPinRef,
  resolveInspectableResult,
  resolveInspectableResultRef,
  resultRef,
} from "./inspectableResult";

const graphPath = "events/Main.yssbi-event";
const output: PortAddressDto = {
  kind: "instance",
  nodeId: "node-1",
  templateKey: "values",
  instanceId: "instance-2",
};

function entry(resultId: string, state: PinResultEntry["state"]): PinResultEntry {
  return {
    resultId,
    runId: `run-${resultId}`,
    activationId: `activation-${resultId}`,
    graphRevision: "7",
    createdAtMs: resultId,
    usage: { kind: "produced" },
    state,
  };
}

function createFixture() {
  const descriptors = new Map<string, DeepReadonly<ResultDescriptor | null>>();
  const values = new Map<string, DeepReadonly<ResultValue | null>>();
  const pages = new Map<string, DeepReadonly<ResultPage | null>>();
  const histories = new Map<string, DeepReadonly<readonly PinResultEntry[]>>();
  const service = {
    getDescriptor: vi.fn(async (_resultId: string): Promise<ResultDescriptor | null> => null),
    getValue: vi.fn(async (_resultId: string): Promise<ResultValue | null> => null),
    getPage: vi.fn(
      async (_resultId: string, _offset: number, _limit: number): Promise<ResultPage | null> =>
        null,
    ),
    getPinHistory: vi.fn(
      async (_graphPath: string, _output: PortAddressDto): Promise<readonly PinResultEntry[]> => [],
    ),
  } satisfies ResultQueryDependencies["service"];
  const key = (value: object): string => JSON.stringify(value);
  const publication: ResultQueryPublication = {
    publishDescriptor: (_projectId, resultId, value) => descriptors.set(resultId, value),
    publishValue: (_projectId, resultId, value) => values.set(resultId, value),
    publishPage: (_projectId, request, value) => pages.set(key(request), value),
    publishPinHistory: (_projectId, request, value) => histories.set(key(request), value),
    publishFailure: () => undefined,
  };
  const read: ResultQueryReadCapability = {
    subscribe: () => () => undefined,
    getDescriptor: (resultId) => descriptors.get(resultId) ?? null,
    getValue: (resultId) => values.get(resultId) ?? null,
    getPage: (request) => pages.get(key(request)) ?? null,
    getPinHistory: (request) => histories.get(key(request)) ?? null,
    getFailure: () => null,
  };
  const dependencies: ResultQueryDependencies = {
    readCurrentProjectInstanceId: () => "project-a",
    service,
    publication,
  };
  return { dependencies, read, service };
}

function queryDependencies(fixture: ReturnType<typeof createFixture>) {
  return {
    coordinator: createResultQueryCoordinator(fixture.dependencies),
    read: fixture.read,
  };
}

describe("resolveInspectableResult", () => {
  beforeEach(() => vi.clearAllMocks());

  it("resolves an exact result ID", async () => {
    const fixture = createFixture();
    fixture.service.getDescriptor.mockResolvedValue(null);
    await expect(
      resolveInspectableResult(resultRef("17"), queryDependencies(fixture)),
    ).resolves.toBeNull();
    expect(fixture.service.getDescriptor).toHaveBeenCalledWith("17");
  });

  it.each([
    ["pending", { kind: "pending", progress: { completed: "0", total: null } }],
    [
      "failed",
      {
        kind: "failed",
        failure: {
          code: "execution_failed",
          cause: { kind: "execution" },
          upstreamResultIds: [],
        },
      },
    ],
    ["cancelled", { kind: "cancelled" }],
  ] satisfies ReadonlyArray<readonly [string, PinResultEntry["state"]]>)(
    "selects the latest occurrence even when it is %s",
    async (_label, state) => {
      const fixture = createFixture();
      fixture.service.getPinHistory.mockResolvedValue([
        entry("17", { kind: "ready" }),
        entry("18", state),
      ]);

      await expect(
        resolveInspectableResultRef(outputPinRef(graphPath, output), queryDependencies(fixture)),
      ).resolves.toEqual({
        ref: resultRef("18"),
        history: expect.objectContaining({
          graphPath,
          output,
          selectedResultId: "18",
        }),
        status: "published",
      });
      expect(fixture.service.getPinHistory).toHaveBeenCalledWith(graphPath, output);
    },
  );

  it("selects an exact historical result ID", async () => {
    const fixture = createFixture();
    fixture.service.getPinHistory.mockResolvedValue([
      entry("17", { kind: "ready" }),
      entry("18", { kind: "cancelled" }),
    ]);

    await expect(
      resolveInspectableResultRef(
        outputPinRef(graphPath, output),
        queryDependencies(fixture),
        "17",
      ),
    ).resolves.toEqual({
      ref: resultRef("17"),
      history: expect.objectContaining({ selectedResultId: "17" }),
      status: "published",
    });
  });

  it("rejects a historical result ID that is not in that output history", async () => {
    const fixture = createFixture();
    fixture.service.getPinHistory.mockResolvedValue([entry("17", { kind: "ready" })]);
    await expect(
      resolveInspectableResultRef(
        outputPinRef(graphPath, output),
        queryDependencies(fixture),
        "99",
      ),
    ).resolves.toEqual({
      ref: null,
      history: expect.objectContaining({ selectedResultId: null }),
      status: "published",
    });
  });
});
