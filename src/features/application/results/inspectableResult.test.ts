import { resultReferenceKey } from "@/shared/types/domain/result";
import type { ResultReference } from "@/shared/types/domain/result";
import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
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

function createFixture() {
  const descriptors = new Map<string, DeepReadonly<ResultDescriptor | null>>();
  const values = new Map<string, DeepReadonly<ResultValue | null>>();
  const pages = new Map<string, DeepReadonly<ResultPage | null>>();
  const pinResults = new Map<string, DeepReadonly<ResultDescriptor | null>>();
  const service = {
    analyze: vi.fn(async () => {
      throw new Error("unexpected analysis");
    }),
    getDescriptor: vi.fn(
      async (_reference: ResultReference): Promise<ResultDescriptor | null> => null,
    ),
    getValue: vi.fn(async (_reference: ResultReference): Promise<ResultValue | null> => null),
    getPage: vi.fn(
      async (
        _reference: ResultReference,
        _offset: number,
        _limit: number,
      ): Promise<ResultPage | null> => null,
    ),
    getPinResult: vi.fn(
      async (_graphPath: string, _output: PortAddressDto): Promise<ResultDescriptor | null> => null,
    ),
  } satisfies ResultQueryDependencies["service"];
  const key = (value: object): string => JSON.stringify(value);
  const publication: ResultQueryPublication = {
    publishAnalysis: vi.fn(),
    releasePayload: vi.fn(),
    publishDescriptor: (_projectId, resultId, value) =>
      descriptors.set(resultReferenceKey(resultId), value),
    publishValue: (_projectId, resultId, value) => values.set(resultReferenceKey(resultId), value),
    publishPage: (_projectId, request, value) => pages.set(key(request), value),
    publishPinResult: (_projectId, request, value) => pinResults.set(key(request), value),
    publishFailure: () => undefined,
  };
  const read: ResultQueryReadCapability = {
    getAnalysis: () => null,
    subscribe: () => () => undefined,
    getDescriptor: (resultId) => descriptors.get(resultReferenceKey(resultId)) ?? null,
    getValue: (resultId) => values.get(resultReferenceKey(resultId)) ?? null,
    getPage: (request) => pages.get(key(request)) ?? null,
    getPinResult: (request) => pinResults.get(key(request)) ?? null,
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
      resolveInspectableResult(resultRef(resultReferenceFixture("17")), queryDependencies(fixture)),
    ).resolves.toBeNull();
    expect(fixture.service.getDescriptor).toHaveBeenCalledWith(resultReferenceFixture("17"));
  });

  it("resolves the current output result and clears it when the backend has no result", async () => {
    const fixture = createFixture();
    const descriptor: ResultDescriptor = {
      resultId: "18",

      executionSessionId: resultSessionFixture,
      provenance: {
        runId: "2",
        createdAtMs: "1000",
        graphPath,
        nodeId: output.nodeId,
        output: { graphPath, port: output },
      },
      presentation: { kind: "inspector" },
      valueKind: "scalar",
      metadata: null,
      totalCount: 1,
      title: "Result",
    };
    fixture.service.getPinResult.mockResolvedValueOnce(descriptor).mockResolvedValueOnce(null);
    const dependencies = queryDependencies(fixture);
    const ref = outputPinRef(graphPath, output);
    await expect(resolveInspectableResultRef(ref, dependencies)).resolves.toEqual({
      ref: resultRef(resultReferenceFixture("18")),
      status: "published",
    });
    await expect(resolveInspectableResultRef(ref, dependencies)).resolves.toEqual({
      ref: null,
      status: "notReady",
    });
    expect(fixture.read.getPinResult({ graphPath, output })).toBeNull();
  });
});
