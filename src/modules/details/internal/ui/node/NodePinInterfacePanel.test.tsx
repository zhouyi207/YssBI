// @vitest-environment happy-dom

import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { ResolvedPinSpec } from "../resolveNodePinSpecs";
import { NodePinInterfacePanel } from "./NodePinInterfacePanel";

const connectPinsById = vi.hoisted(() => vi.fn());
const disconnectConnectionById = vi.hoisted(() => vi.fn());
const disconnectPinById = vi.hoisted(() => vi.fn());

vi.mock("@/features/application/editor/edgeOperations", () => ({
  connectPinsById,
  disconnectConnectionById,
  disconnectPinById,
}));

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "detail.nodeDoc.inputs": "Inputs",
        "detail.nodeDoc.outputs": "Outputs",
        "detail.nodeDoc.noInputs": "No inputs",
        "detail.nodeDoc.noOutputs": "No outputs",
        "detail.nodeDoc.selectInput": "Select input",
        "detail.nodeDoc.addConnection": "Add connection",
        "detail.nodeDoc.removeConnection": "Remove connection",
        "detail.nodeDoc.unconnected": "Not connected",
      })[key] ?? key,
  }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const input: ResolvedPinSpec = {
  id: "input-value",
  name: "Value",
  direction: "input",
  kind: "Data",
  typeLabel: "Float64",
  optional: false,
  connected: false,
  connectionIds: [],
};

const output: ResolvedPinSpec = {
  id: "output-result",
  name: "Result",
  direction: "output",
  kind: "Data",
  typeLabel: "Float64",
  optional: false,
  connected: false,
  connectionIds: [],
};

const graphPath = "events/Main.yssbi-event";

function makePin(
  id: string,
  nodeId: string,
  name: string,
  direction: "input" | "output",
  kind: "data" | "control" = "data",
) {
  return {
    id,
    nodeId,
    name,
    type: kind === "control" ? ("exec" as const) : ("object" as const),
    direction,
    dataType: kind === "control" ? undefined : { kind: "Float64" as const },
    kind,
    connections: {
      current: 0,
      maximum: null,
      ordered: false,
      canAppend: true,
      canReplace: true,
      canMove: true,
    },
  };
}

function graphBucket(): GraphEntityBucket {
  return {
    basis: {
      graphPath,
      graphRevision: 1,
      registryFingerprint: "fingerprint",
      resourceVersions: {},
    },
    sourceRevision: 1,
    requestGeneration: 1,
    diagnostics: [],
    outcome: { type: "success" },
    hasBlockingDiagnostics: false,
    nodes: {
      current: {
        id: "current",
        graphPath,
        nodeType: "test.current",
        category: [],
        title: "Current node",
        inputs: [input.id],
        outputs: [output.id],
        position: { x: 0, y: 0 },
      },
      source: {
        id: "source",
        graphPath,
        nodeType: "test.source",
        category: [],
        title: "Source node",
        inputs: [],
        outputs: ["source-output"],
        position: { x: 0, y: 0 },
      },
      target: {
        id: "target",
        graphPath,
        nodeType: "test.target",
        category: [],
        title: "Target node",
        inputs: ["target-input"],
        outputs: [],
        position: { x: 0, y: 0 },
      },
    },
    pins: {
      [input.id]: makePin(input.id, "current", input.name, "input"),
      [output.id]: makePin(output.id, "current", output.name, "output"),
      "source-output": makePin("source-output", "source", "Result", "output"),
      "target-input": makePin("target-input", "target", "Value", "input"),
      "exec-input": makePin("exec-input", "target", "Exec", "input", "control"),
    },
    connections: {},
    graphNodes: ["current", "source", "target"],
    nodePins: {
      current: [input.id, output.id],
      source: ["source-output"],
      target: ["target-input", "exec-input"],
    },
    pinConnections: {
      [input.id]: [],
      [output.id]: [],
      "source-output": [],
      "target-input": [],
      "exec-input": [],
    },
  };
}

function chooseSelectItem(item: HTMLElement | undefined): void {
  if (!item) return;
  item.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerType: "mouse" }));
  item.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerType: "mouse" }));
}

afterEach(() => {
  document.body.replaceChildren();
  useGraphProjectionStore.setState({ graphEntities: {} });
});

beforeEach(() => {
  vi.clearAllMocks();
  connectPinsById.mockResolvedValue({ status: "applied", result: {} });
  disconnectConnectionById.mockResolvedValue({ status: "applied", result: {} });
  disconnectPinById.mockResolvedValue({ status: "applied", result: {} });
});

describe("NodePinInterfacePanel", () => {
  it("renders pin selectors with compatible graph options", async () => {
    useGraphProjectionStore.setState({ graphEntities: { [graphPath]: graphBucket() } });
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(NodePinInterfacePanel, {
          graphPath,
          inputs: [input],
          outputs: [output],
        }),
      );
    });

    expect(container.querySelectorAll('[data-slot="collapsible"]')).toHaveLength(2);
    expect(
      Array.from(container.querySelectorAll('[data-slot="collapsible-trigger"]')).map((trigger) =>
        trigger.textContent?.replace(/\s+/g, " ").trim(),
      ),
    ).toEqual(["Inputs", "Outputs"]);

    const sections = Array.from(
      container.querySelectorAll<HTMLElement>('[data-slot="collapsible"]'),
    );
    expect(sections[0]?.getAttribute("data-state")).toBe("closed");
    expect(sections[1]?.getAttribute("data-state")).toBe("closed");

    await act(async () => {
      container
        .querySelectorAll<HTMLElement>('[data-slot="collapsible-trigger"]')[0]
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(container.querySelectorAll('[data-slot="select-trigger"]')).toHaveLength(1);

    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-slot="select-trigger"]')
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(
      Array.from(document.querySelectorAll('[data-slot="select-item"]')).map((item) =>
        item.textContent?.trim(),
      ),
    ).toContain("Source node · Result");
    expect(
      Array.from(document.querySelectorAll('[data-slot="select-item"]')).map((item) =>
        item.textContent?.trim(),
      ),
    ).not.toContain("Target node · Exec");

    await act(async () => {
      container
        .querySelectorAll<HTMLElement>('[data-slot="collapsible-trigger"]')[1]
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(container.querySelectorAll('[data-slot="select-trigger"]')).toHaveLength(2);

    const addButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="add-output-connection-output-result"]',
    );
    await act(async () => addButton?.click());
    expect(container.querySelectorAll('[data-slot="select-trigger"]')).toHaveLength(3);
    expect(
      container.querySelectorAll('[data-testid^="remove-output-connection-output-result"]'),
    ).toHaveLength(2);

    await act(async () => {
      container
        .querySelectorAll<HTMLElement>('[data-testid^="remove-output-connection-output-result"]')[1]
        ?.click();
    });
    expect(container.querySelectorAll('[data-slot="select-trigger"]')).toHaveLength(2);

    await act(async () => root.unmount());
  });

  it("uses the output slots to add and remove exact graph connections", async () => {
    useGraphProjectionStore.setState({ graphEntities: { [graphPath]: graphBucket() } });
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(NodePinInterfacePanel, {
          graphPath,
          inputs: [],
          outputs: [output],
        }),
      );
    });
    await act(async () => {
      container
        .querySelectorAll<HTMLElement>('[data-slot="collapsible-trigger"]')[1]
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-slot="select-trigger"]')
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const outputItems = Array.from(
      document.querySelectorAll<HTMLElement>('[data-slot="select-item"]'),
    );
    const targetItem = outputItems.find(
      (item) => item.textContent?.trim() === "Target node · Value",
    );
    await act(async () => {
      chooseSelectItem(targetItem);
    });
    expect(connectPinsById).toHaveBeenCalledWith(graphPath, output.id, "target-input");

    const connectedBucket = graphBucket();
    connectedBucket.connections["edge-1"] = {
      id: "edge-1",
      from: output.id,
      to: "target-input",
    };
    connectedBucket.pinConnections[output.id] = ["edge-1"];
    connectedBucket.pinConnections["target-input"] = ["edge-1"];
    await act(async () => {
      useGraphProjectionStore.setState({ graphEntities: { [graphPath]: connectedBucket } });
      root.render(
        createElement(NodePinInterfacePanel, {
          graphPath,
          inputs: [],
          outputs: [output],
        }),
      );
    });
    expect(
      container.querySelector('[data-testid="remove-output-connection-output-result-0"]'),
    ).not.toBeNull();
    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-testid="remove-output-connection-output-result-0"]')
        ?.click();
    });
    expect(disconnectConnectionById).toHaveBeenCalledWith(graphPath, "edge-1");

    await act(async () => root.unmount());
  });

  it("does not expose a raw node id when a connection target lacks its node projection", async () => {
    const bucket = graphBucket();
    delete bucket.nodes.target;
    useGraphProjectionStore.setState({ graphEntities: { [graphPath]: bucket } });
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(NodePinInterfacePanel, {
          graphPath,
          inputs: [],
          outputs: [output],
        }),
      );
    });
    await act(async () => {
      container
        .querySelectorAll<HTMLElement>('[data-slot="collapsible-trigger"]')[1]
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-slot="select-trigger"]')
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    const items = Array.from(document.querySelectorAll<HTMLElement>('[data-slot="select-item"]'));
    expect(items.map((item) => item.textContent?.trim())).toContain("Value");
    expect(items.map((item) => item.textContent?.trim())).not.toContain("target · Value");

    await act(async () => root.unmount());
  });

  it("uses the input selector to connect or clear its upstream output", async () => {
    useGraphProjectionStore.setState({ graphEntities: { [graphPath]: graphBucket() } });
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(NodePinInterfacePanel, {
          graphPath,
          inputs: [input],
          outputs: [],
        }),
      );
    });
    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-slot="collapsible-trigger"]')
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-slot="select-trigger"]')
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    const inputItems = Array.from(
      document.querySelectorAll<HTMLElement>('[data-slot="select-item"]'),
    );
    const sourceItem = inputItems.find(
      (item) => item.textContent?.trim() === "Source node · Result",
    );
    await act(async () => {
      chooseSelectItem(sourceItem);
    });
    expect(connectPinsById).toHaveBeenCalledWith(graphPath, "source-output", input.id);

    const connectedBucket = graphBucket();
    connectedBucket.connections["edge-2"] = {
      id: "edge-2",
      from: "source-output",
      to: input.id,
    };
    connectedBucket.pinConnections[input.id] = ["edge-2"];
    connectedBucket.pinConnections["source-output"] = ["edge-2"];
    await act(async () => {
      useGraphProjectionStore.setState({ graphEntities: { [graphPath]: connectedBucket } });
      root.render(
        createElement(NodePinInterfacePanel, {
          graphPath,
          inputs: [input],
          outputs: [],
        }),
      );
    });
    await act(async () => {
      container
        .querySelector<HTMLElement>('[data-slot="select-trigger"]')
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const emptyItem = Array.from(
      document.querySelectorAll<HTMLElement>('[data-slot="select-item"]'),
    ).find((item) => item.textContent?.trim() === "Not connected");
    await act(async () => {
      chooseSelectItem(emptyItem);
    });
    expect(disconnectPinById).toHaveBeenCalledWith(graphPath, input.id);

    await act(async () => root.unmount());
  });
});
