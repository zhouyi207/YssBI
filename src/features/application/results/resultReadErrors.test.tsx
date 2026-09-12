// @vitest-environment happy-dom
import { resultReferenceKey } from "@/shared/types/domain/result";
import type { ResultReference } from "@/shared/types/domain/result";
import { resultReferenceFixture } from "@/tests/helpers/resultFixture";
import type { ReactNode } from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ResultReadError } from "./components/ResultReadError";
import { useResultValue } from "./useResultValue";
import {
  createResultQueryCoordinator,
  type ResultPageRequest,
  type ResultPinRequest,
  type ResultQueryReadCapability,
  type ResultQueryScope,
  type ResultQueryServicePort,
} from "./resultQueryCoordinator";
import type { ErrorReference } from "@/features/application/errorReference";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ResultPage, ResultValue } from "./types";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => `localized:${key}` }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type TestRuntime = {
  coordinator: ReturnType<typeof createResultQueryCoordinator>;
  read: ResultQueryReadCapability;
  service: ResultQueryServicePort & {
    getValue: ReturnType<typeof vi.fn>;
    getPage: ReturnType<typeof vi.fn>;
  };
};

let runtime: TestRuntime;
let valueState: ReturnType<typeof useResultValue> | undefined;

function pageKey(request: ResultPageRequest): string {
  return `${request.resultId}:${request.offset}:${request.limit}`;
}

function scopeKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case "analysis":
      return `analysis:${scope.reference.resultId}:${scope.analysis.kind}`;
    case "descriptor":
    case "value":
      return `${scope.kind}:${scope.resultId}`;
    case "page":
      return `page:${pageKey(scope)}`;
    case "pinResult":
      return `pinResult:${scope.graphPath}:${JSON.stringify(scope.output)}`;
  }
}

function createTestRuntime(): TestRuntime {
  let projectInstanceId: string | null = "project-1";
  const values = new Map<string, DeepReadonly<ResultValue | null>>();
  const pages = new Map<string, DeepReadonly<ResultPage | null>>();
  const failures = new Map<string, ErrorReference>();
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((listener) => listener());

  const service = {
    analyze: vi.fn(async () => {
      throw new Error("unexpected analysis");
    }),
    getDescriptor: vi.fn(async (_reference: ResultReference) => null),
    getValue: vi.fn(async (_reference: ResultReference): Promise<ResultValue | null> => null),
    getPage: vi.fn(
      async (
        _reference: ResultReference,
        _offset: number,
        _limit: number,
      ): Promise<ResultPage | null> => null,
    ),
    getPinResult: vi.fn(async (_graphPath: string, _output: ResultPinRequest["output"]) => null),
  };

  const read: ResultQueryReadCapability = {
    getAnalysis: () => null,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getDescriptor: () => null,
    getValue: (resultId) => values.get(resultReferenceKey(resultId)) ?? null,
    getPage: (request) => pages.get(pageKey(request)) ?? null,
    getPinResult: () => null,
    getFailure: (scope) => failures.get(scopeKey(scope)) ?? null,
  };

  const coordinator = createResultQueryCoordinator({
    readCurrentProjectInstanceId: () => projectInstanceId,
    service,
    publication: {
      publishAnalysis: () => undefined,
      releasePayload: (resultId) => {
        values.delete(resultReferenceKey(resultId));
        for (const key of pages.keys()) if (key.startsWith(`${resultId}:`)) pages.delete(key);
        notify();
      },
      publishDescriptor: () => undefined,
      publishValue: (_project, resultId, value) => {
        values.set(resultReferenceKey(resultId), value);
        notify();
      },
      publishPage: (_project, request, page) => {
        pages.set(pageKey(request), page);
        notify();
      },
      publishPinResult: () => undefined,
      publishFailure: (_project, scope, issue) => {
        failures.set(scopeKey(scope), issue);
        notify();
      },
    },
  });

  return {
    coordinator,
    read,
    service: service as TestRuntime["service"],
  };
}

function ValueHarness({ showError = false }: { showError?: boolean }) {
  valueState = useResultValue(resultReferenceFixture("42"), runtime);
  return showError && valueState.error ? <ResultReadError error={valueState.error} /> : null;
}

async function flushAsyncWork() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe("result read machine errors", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    runtime = createTestRuntime();
    valueState = undefined;
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  async function render(ui: ReactNode) {
    await act(async () => {
      root.render(ui);
      await flushAsyncWork();
    });
  }

  it("retains shared payload until the final mounted consumer releases it", async () => {
    runtime.service.getValue.mockResolvedValue({ kind: "value", value: 42 });
    await render(
      <div>
        <ValueHarness key="a" />
        <ValueHarness key="b" />
      </div>,
    );
    expect(valueState?.value).toEqual({ kind: "value", value: 42 });
    await render(
      <div>
        <ValueHarness key="b" />
      </div>,
    );
    expect(valueState?.value).toEqual({ kind: "value", value: 42 });
    expect(runtime.read.getValue(resultReferenceFixture("42"))).toEqual({
      kind: "value",
      value: 42,
    });
    await render(null);
    expect(runtime.read.getValue(resultReferenceFixture("42"))).toBeNull();
  });
});
