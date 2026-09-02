// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { markResourceLoaded, useDocumentStateStore } from "@/features/core/resource";
import { useExecutionStore } from "@/features/core/execution";
import { PinPreviewGenerationService } from "@/services/nodeSystem/pinPreviewGenerationService";
import { ProjectService } from "@/services/project/projectService";
import { ResultService } from "@/services/result/resultService";
import { TooltipProvider } from "@/components/ui/tooltip";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { resetResultQueryProject } from "@/features/application/results";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { GraphPinController } from "./GraphPinController";
import { pinConnectionFeedbackAttributes } from "./GraphPinView";

const katexWarningSpy = vi.hoisted(() => {
  const warn = console.warn.bind(console);
  return vi.spyOn(console, "warn").mockImplementation((message, ...args) => {
    const quirksWarning =
      "Warning: KaTeX doesn't work in quirks mode. Make sure your website has a suitable doctype.";
    if (message !== quirksWarning) warn(message, ...args);
  });
});

vi.mock("@/features/application/window", () => ({
  openPresentationWindow: vi.fn(),
  presentationWindowPayload: vi.fn(() => ({})),
  presentationWindowPayloadFromDescriptor: vi.fn(() => ({})),
}));

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Main.yssbi-event";

describe("Pin connection feedback", () => {
  it("maps structured feedback to safe metadata", () => {
    expect(pinConnectionFeedbackAttributes({ kind: "append" })).toEqual({
      "data-connection-feedback": "append",
    });
    expect(pinConnectionFeedbackAttributes({ kind: "invalid", invalidReason: "capacity" })).toEqual(
      {
        "data-connection-feedback": "invalid",
        "data-connection-invalid-reason": "capacity",
      },
    );
  });
});

describe("Pin preview production path", () => {
  afterAll(() => katexWarningSpy.mockRestore());
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.restoreAllMocks();
    resetResultQueryProject();
    useProjectIOStore.setState({ projectInstanceId: "project-session-1" });
    clearProjectLifecycle();
    startProjectLifecycle("project-session-1");
    useGraphProjectionStore.setState({ graphEntities: {} });
    useGraphSessionStore.getState().reset();
    useDocumentStateStore.getState().clear();
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
    vi.spyOn(PinPreviewGenerationService, "allocate").mockResolvedValue(1);
    vi.spyOn(ResultService, "getPinHistory").mockResolvedValue([]);
    vi.spyOn(ResultService, "getDescriptor").mockResolvedValue(null);
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelector("[data-yssbi-overlay-root]")?.remove();
    resetResultQueryProject();
    useProjectIOStore.setState({ projectInstanceId: null });
    vi.restoreAllMocks();
  });

  it("routes output View through authoritative structured Pin history", async () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    expect(
      useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection, 1)
        .applied,
    ).toBe(true);
    markResourceLoaded({ id: graphPath, kind: "event" });
    useGraphSessionStore.getState().setFocusedSession("editor-a", graphPath);
    const pin = useGraphProjectionStore.getState().getGraphPin(graphPath, fixture.outputKey);
    if (!pin) throw new Error("expected projected output pin");
    const execute = vi.spyOn(ProjectService, "executeGraphDocument");

    act(() =>
      root.render(
        <TooltipProvider>
          <GraphPinController {...pin} graphPath={graphPath} />
        </TooltipProvider>,
      ),
    );
    const pinElement = container.querySelector(`[data-pin-id="${fixture.outputKey}"]`);
    if (!pinElement) throw new Error("expected rendered pin");
    act(() => {
      pinElement.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
          clientX: 10,
          clientY: 20,
        }),
      );
    });

    const viewItem = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((item) =>
      item.textContent?.includes("contextMenu.pin.view"),
    );
    expect(viewItem).toBeDefined();
    expect(viewItem?.hasAttribute("data-disabled")).toBe(false);

    await act(async () => {
      viewItem?.click();
      await Promise.resolve();
    });

    expect(ResultService.getPinHistory).toHaveBeenCalledWith(graphPath, fixture.outputAddress);
    expect(execute).not.toHaveBeenCalled();
  });

  it("opens an exact historical occurrence from the compact Pin context menu", async () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    expect(
      useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection, 1)
        .applied,
    ).toBe(true);
    const pin = useGraphProjectionStore.getState().getGraphPin(graphPath, fixture.outputKey);
    if (!pin) throw new Error("expected projected output pin");
    vi.mocked(ResultService.getPinHistory).mockResolvedValue([
      {
        resultId: "17",
        runId: "7",
        activationId: "70",
        graphRevision: "1",
        createdAtMs: "1000",
        usage: { kind: "produced" },
        state: { kind: "ready" },
      },
      {
        resultId: "18",
        runId: "8",
        activationId: "80",
        graphRevision: "1",
        createdAtMs: "2000",
        usage: { kind: "produced" },
        state: { kind: "cancelled" },
      },
    ]);

    act(() =>
      root.render(
        <TooltipProvider>
          <GraphPinController {...pin} graphPath={graphPath} />
        </TooltipProvider>,
      ),
    );
    const pinElement = container.querySelector(`[data-pin-id="${fixture.outputKey}"]`);
    if (!pinElement) throw new Error("expected rendered pin");
    const openContext = () =>
      act(() => {
        pinElement.dispatchEvent(
          new MouseEvent("contextmenu", {
            bubbles: true,
            cancelable: true,
            clientX: 10,
            clientY: 20,
          }),
        );
      });

    openContext();
    const viewItem = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((item) =>
      item.textContent?.includes("contextMenu.pin.view"),
    );
    await act(async () => {
      viewItem?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    openContext();
    const historical = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (item) => item.textContent?.includes("17 · ready"),
    );
    expect(historical).toBeDefined();
    await act(async () => {
      historical?.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(ResultService.getDescriptor).toHaveBeenCalledWith("17");
    expect(
      useExecutionStore.getState().getGraph(graphPath).pinHistories.values().next().value,
    ).toMatchObject({ selectedResultId: "17" });
  });

  it("enables authoritative history View for a Function output", async () => {
    const functionPath = "functions/Helper.yssbi-function";
    const fixture = makeEditorProjectionFixture({ graphPath: functionPath });
    expect(
      useGraphProjectionStore.getState().replaceProjection(functionPath, fixture.projection, 1)
        .applied,
    ).toBe(true);
    markResourceLoaded({ id: functionPath, kind: "function" });
    useGraphSessionStore.getState().setFocusedSession("editor-a", functionPath);
    const pin = useGraphProjectionStore.getState().getGraphPin(functionPath, fixture.outputKey);
    if (!pin) throw new Error("expected projected function output pin");
    const execute = vi.spyOn(ProjectService, "executeGraphDocument");

    act(() =>
      root.render(
        <TooltipProvider>
          <GraphPinController {...pin} graphPath={functionPath} />
        </TooltipProvider>,
      ),
    );
    const pinElement = container.querySelector(`[data-pin-id="${fixture.outputKey}"]`);
    if (!pinElement) throw new Error("expected rendered pin");
    act(() => {
      pinElement.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
          clientX: 10,
          clientY: 20,
        }),
      );
    });

    const viewItem = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((item) =>
      item.textContent?.includes("contextMenu.pin.view"),
    );
    expect(viewItem?.hasAttribute("data-disabled")).toBe(false);
    await act(async () => {
      viewItem?.click();
      await Promise.resolve();
    });
    expect(ResultService.getPinHistory).toHaveBeenCalledWith(functionPath, fixture.outputAddress);
    expect(execute).not.toHaveBeenCalled();
  });
});
