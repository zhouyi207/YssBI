import type { ResultReference } from "@/shared/types/domain/result";
import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { describe, expect, it } from "vitest";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { ResultDescriptor, ResultPage, ResultValue } from "@/shared/types/domain/result";
import {
  createResultQueryCoordinator,
  type ResultQueryDependencies,
  type ResultQueryPublication,
} from "./resultQueryCoordinator";

interface Deferred<T> {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((settle, fail) => {
    resolve = settle;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const output: PortAddressDto = {
  kind: "declared",
  nodeId: "00000000-0000-0000-0000-000000000002",
  portKey: "result",
};

const descriptor = {
  resultId: "17",

  executionSessionId: resultSessionFixture,
  provenance: {
    runId: "1",
    graphPath: "events/contract.yssbi-event",
    nodeId: "00000000-0000-0000-0000-000000000002",
    output: { graphPath: "events/contract.yssbi-event", port: output },
    createdAtMs: "4",
  },
  presentation: { kind: "inspector" as const },
  valueKind: "scalar" as const,
  metadata: null,
  totalCount: 1,
  title: "Result",
} satisfies ResultDescriptor;

function page(resultId: string, offset: number, value: number): ResultPage {
  return {
    resultId,
    offset,
    requestedLimit: 2,
    actualCount: 1,
    totalCount: 3,
    hasMore: offset < 2,
    nextOffset: offset < 2 ? offset + 2 : null,
    valueKind: "dataSeries",
    metadata: null,
    values: [value],
  };
}

type TestService = {
  -readonly [
    Key in keyof ResultQueryDependencies["service"]
  ]: ResultQueryDependencies["service"][Key];
};

function setup(): {
  readonly dependencies: ResultQueryDependencies;
  readonly coordinator: ReturnType<typeof createResultQueryCoordinator>;
  readonly service: TestService;
  readonly currentProject: { id: string | null };
  readonly publication: ResultQueryPublication & {
    readonly descriptors: ResultDescriptor[];
    readonly values: ResultValue[];
    readonly pages: ResultPage[];
    readonly pinResults: (ResultDescriptor | null)[];
    readonly failures: Array<{ readonly scopeKind: string; readonly issueCode: string }>;
  };
} {
  const currentProject = { id: "project-a" as string | null };
  const descriptors: ResultDescriptor[] = [];
  const values: ResultValue[] = [];
  const pages: ResultPage[] = [];
  const pinResults: (ResultDescriptor | null)[] = [];
  const failures: Array<{ readonly scopeKind: string; readonly issueCode: string }> = [];
  const publication: ResultQueryPublication & {
    readonly descriptors: ResultDescriptor[];
    readonly values: ResultValue[];
    readonly pages: ResultPage[];
    readonly pinResults: (ResultDescriptor | null)[];
    readonly failures: Array<{ readonly scopeKind: string; readonly issueCode: string }>;
  } = {
    descriptors,
    values,
    pages,
    pinResults,
    failures,
    releasePayload: () => undefined,
    publishAnalysis: () => undefined,
    publishDescriptor: (_projectId, _resultId, value) => {
      if (value) descriptors.push(value as ResultDescriptor);
    },
    publishValue: (_projectId, _resultId, value) => {
      if (value) values.push(value as ResultValue);
    },
    publishPage: (_projectId, _request, value) => {
      if (value) pages.push(value as ResultPage);
    },
    publishPinResult: (_projectId, _request, entries) => {
      pinResults.push(entries as ResultDescriptor | null);
    },
    publishFailure: (_projectId, scope, issue) => {
      failures.push({ scopeKind: scope.kind, issueCode: issue.code });
    },
  };
  const service: TestService = {
    analyze: async () => {
      throw new Error("unexpected analysis");
    },
    getDescriptor: async () => descriptor,
    getValue: async () => ({ kind: "value", value: 4 }) as ResultValue,
    getPage: async (_reference: ResultReference, offset: number) => page("17", offset, offset),
    getPinResult: async () => descriptor,
  };
  const dependencies: ResultQueryDependencies = {
    readCurrentProjectInstanceId: () => currentProject.id,
    service,
    publication,
  };
  return {
    dependencies,
    coordinator: createResultQueryCoordinator(dependencies),
    service,
    currentProject,
    publication,
  };
}

describe("ResultQueryCoordinator", () => {
  it("isolates report tables and discards analysis results after their result is invalidated", async () => {
    const fixture = setup();
    const reference = {
      executionSessionId: "00000000-0000-0000-0000-000000000001",
      resultId: "17",
    };
    const coefficients = deferred<ResultPage | null>();
    const observations = deferred<ResultPage | null>();
    fixture.service.getPage = (_id, _offset, _limit, part) =>
      part === "coefficients" ? coefficients.promise : observations.promise;
    const first = fixture.coordinator.loadPage({
      executionSessionId: resultSessionFixture,
      resultId: "17",
      offset: 0,
      limit: 2,
      part: "coefficients",
    });
    const second = fixture.coordinator.loadPage({
      executionSessionId: resultSessionFixture,
      resultId: "17",
      offset: 0,
      limit: 2,
      part: "observations",
    });
    observations.resolve(page("17", 0, 3));
    coefficients.resolve(page("17", 0, 7));
    expect(await first).toEqual({ status: "published" });
    expect(await second).toEqual({ status: "published" });
    expect(fixture.publication.pages).toHaveLength(2);

    const analysis = deferred<Awaited<ReturnType<TestService["analyze"]>>>();
    fixture.service.analyze = () => analysis.promise;
    const pending = fixture.coordinator.loadAnalysis({
      reference,
      analysis: { kind: "acfPacf", maxLag: 1 },
    });
    fixture.coordinator.resetResult(resultReferenceFixture("17"));
    analysis.resolve({ kind: "acfPacf", value: { acf: [1, 0.5], pacf: [0.5], n: 53940 } });
    expect(await pending).toEqual({ status: "stale" });
  });

  it("drops stale success and failure after project replacement", async () => {
    const fixture = setup();
    const staleSuccess = deferred<ResultPage | null>();
    const staleFailure = deferred<ResultValue | null>();
    fixture.service.getPage = () => staleSuccess.promise;
    fixture.service.getValue = () => staleFailure.promise;

    const success = fixture.coordinator.loadPage({
      executionSessionId: resultSessionFixture,
      resultId: "17",
      offset: 0,
      limit: 2,
    });
    const failure = fixture.coordinator.loadValue({
      executionSessionId: resultSessionFixture,
      resultId: "17",
    });
    fixture.currentProject.id = "project-b";
    fixture.coordinator.resetProject();

    staleSuccess.resolve(page("17", 0, 99));
    staleFailure.reject(new Error("secret transport text"));

    await expect(success).resolves.toEqual({ status: "stale" });
    await expect(failure).resolves.toEqual({ status: "stale" });
    expect(fixture.publication.pages).toEqual([]);
    expect(fixture.publication.failures).toEqual([]);
  });

  it("publishes only the latest requested page for a result", async () => {
    const fixture = setup();
    const oldPage = deferred<ResultPage | null>();
    const newPage = deferred<ResultPage | null>();
    const otherPage = deferred<ResultPage | null>();
    const requests = new Map<number, Deferred<ResultPage | null>[]>([
      [0, [oldPage, newPage]],
      [2, [otherPage]],
    ]);
    fixture.service.getPage = async (_resultId, offset) =>
      requests.get(offset)?.shift()?.promise ?? null;

    const first = fixture.coordinator.loadPage({
      executionSessionId: resultSessionFixture,
      resultId: "17",
      offset: 0,
      limit: 2,
    });
    const second = fixture.coordinator.loadPage({
      executionSessionId: resultSessionFixture,
      resultId: "17",
      offset: 0,
      limit: 2,
    });
    const independent = fixture.coordinator.loadPage({
      executionSessionId: resultSessionFixture,
      resultId: "17",
      offset: 2,
      limit: 2,
    });

    newPage.resolve(page("17", 0, 2));
    otherPage.resolve(page("17", 2, 3));
    oldPage.resolve(page("17", 0, 1));

    await expect(second).resolves.toEqual({ status: "stale" });
    await expect(independent).resolves.toEqual({ status: "published" });
    await expect(first).resolves.toEqual({ status: "stale" });
    expect(fixture.publication.pages.map((value) => value.values[0])).toEqual([3]);
  });

  it("publishes every typed query through the matching publication and safely maps failures", async () => {
    const fixture = setup();
    await expect(
      fixture.coordinator.loadDescriptor({
        executionSessionId: resultSessionFixture,
        resultId: "17",
      }),
    ).resolves.toEqual({
      status: "published",
    });
    await expect(
      fixture.coordinator.loadValue({ executionSessionId: resultSessionFixture, resultId: "17" }),
    ).resolves.toEqual({
      status: "published",
    });
    await expect(
      fixture.coordinator.loadPinResult({
        graphPath: "events/contract.yssbi-event",
        output,
      }),
    ).resolves.toEqual({ status: "published" });

    fixture.service.getValue = async () => {
      throw new Error("secret transport text");
    };
    await expect(
      fixture.coordinator.loadValue({ executionSessionId: resultSessionFixture, resultId: "17" }),
    ).resolves.toEqual({
      status: "failed",
    });
    expect(fixture.publication.descriptors).toHaveLength(1);
    expect(fixture.publication.values).toHaveLength(1);
    expect(fixture.publication.pinResults).toHaveLength(1);
    expect(fixture.publication.failures).toEqual([
      { scopeKind: "value", issueCode: "result_value_read_failed" },
    ]);
  });
});
