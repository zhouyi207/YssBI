// @vitest-environment happy-dom
import type { ReactNode } from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ResultReadError } from "./components/ResultReadError";
import { usePagedResultRows } from "./usePagedResultRows";
import { useResultValue } from "./useResultValue";
import {
  createResultQueryCoordinator,
  type ResultPageRequest,
  type ResultPinHistoryRequest,
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
let pageState: ReturnType<typeof usePagedResultRows> | undefined;

function pageKey(request: ResultPageRequest): string {
  return `${request.resultId}:${request.offset}:${request.limit}`;
}

function scopeKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case "descriptor":
    case "value":
      return `${scope.kind}:${scope.resultId}`;
    case "page":
      return `page:${pageKey(scope)}`;
    case "pinHistory":
      return `pinHistory:${scope.graphPath}:${JSON.stringify(scope.output)}`;
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
    getDescriptor: vi.fn(async (_resultId: string) => null),
    getValue: vi.fn(async (_resultId: string): Promise<ResultValue | null> => null),
    getPage: vi.fn(
      async (_resultId: string, _offset: number, _limit: number): Promise<ResultPage | null> =>
        null,
    ),
    getPinHistory: vi.fn(
      async (_graphPath: string, _output: ResultPinHistoryRequest["output"]) => [],
    ),
  };

  const read: ResultQueryReadCapability = {
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getDescriptor: () => null,
    getValue: (resultId) => values.get(resultId) ?? null,
    getPage: (request) => pages.get(pageKey(request)) ?? null,
    getPinHistory: () => null,
    getFailure: (scope) => failures.get(scopeKey(scope)) ?? null,
  };

  const coordinator = createResultQueryCoordinator({
    readCurrentProjectInstanceId: () => projectInstanceId,
    service,
    publication: {
      publishDescriptor: () => undefined,
      publishValue: (_project, resultId, value) => {
        values.set(resultId, value);
        notify();
      },
      publishPage: (_project, request, page) => {
        pages.set(pageKey(request), page);
        notify();
      },
      publishPinHistory: () => undefined,
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
  valueState = useResultValue("42", runtime);
  return showError && valueState.error ? <ResultReadError error={valueState.error} /> : null;
}

function PageHarness() {
  pageState = usePagedResultRows("42", 1, 200, runtime);
  return null;
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
    pageState = undefined;
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

  it("stores a stable value fallback for parser failures without parser prose", async () => {
    runtime.service.getValue.mockRejectedValueOnce(
      new Error("Invalid result value: private response"),
    );

    await render(<ValueHarness />);

    expect(valueState).toMatchObject({
      loading: false,
      error: { code: "result_value_read_failed", incidentId: null },
    });
    expect(JSON.stringify(valueState)).not.toContain("Invalid result value");
    expect(JSON.stringify(valueState)).not.toContain("private response");
  });

  it("stores a distinct page fallback for parser failures without parser prose", async () => {
    runtime.service.getPage.mockRejectedValueOnce(
      new Error("Invalid result page: private page response"),
    );

    await render(<PageHarness />);

    expect(pageState).toMatchObject({
      loading: false,
      error: { code: "result_page_read_failed", incidentId: null },
    });
    expect(JSON.stringify(pageState)).not.toContain("Invalid result page");
    expect(JSON.stringify(pageState)).not.toContain("private page response");
  });

  it("renders a localized generic error and transport code without raw transport text", async () => {
    runtime.service.getValue.mockRejectedValueOnce({
      code: "ipc_transport_failure",
      incidentId: null,
    });

    await render(<ValueHarness showError />);

    const alert = host.querySelector('[role="alert"]');
    expect(alert?.textContent).toContain("localized:resultSource.readFailed");
    expect(alert?.textContent).toContain("localized:common.errorCode");
    expect(alert?.textContent).toContain("ipc_transport_failure");
    expect(alert?.textContent).not.toContain("private native transport failure");
    expect(alert?.textContent).not.toContain("localized:common.incidentId");
  });

  it("renders IPC code and incident ID without backend details or synthesized Error.message", async () => {
    runtime.service.getValue.mockRejectedValueOnce({
      code: "result_value_unavailable",
      incidentId: "incident-result-42",
    });

    await render(<ValueHarness showError />);

    const text = host.querySelector('[role="alert"]')?.textContent;
    expect(text).toContain("localized:resultSource.readFailed");
    expect(text).toContain("result_value_unavailable");
    expect(text).toContain("localized:common.incidentId");
    expect(text).toContain("incident-result-42");
    expect(text).not.toContain("private backend detail");
    expect(text).not.toContain("IPC command 'get_result_value' failed");
  });
});
