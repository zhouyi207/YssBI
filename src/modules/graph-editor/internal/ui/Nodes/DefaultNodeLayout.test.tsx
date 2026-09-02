// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UINode } from "@/features/core/dataStore/nodeView";
import { makeProjectedPinData } from "@/tests/helpers/editorProjectionFixtures";
import { DefaultNodeLayout } from "./DefaultNodeLayout";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ i18n: { resolvedLanguage: "en-US", language: "en" } }),
}));

vi.mock("../Pins/GraphPinController", () => ({
  GraphPinController: ({ pin }: { pin: { name: string } }) => (
    <span data-testid="pin-name">{pin.name}</span>
  ),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function projectedNode(): UINode {
  return {
    id: "database-node",
    nodeType: "yssbi.dataframe.source.get",
    title: "Sales Database",
    styleId: "builtin.default",
    position: { x: 0, y: 0 },
    display: {
      title: "Sales Database",
      userLabel: "Prior period",
      iconId: null,
      styleId: null,
    },
    parameterEditors: [],
    diagnostics: [],
    inputs: [],
    outputs: [
      {
        ...makeProjectedPinData({
          id: "resource-output",
          nodeId: "database-node",
          name: "amount",
          direction: "output",
        }),
        connected: false,
        linkCount: 0,
        connectionIds: [],
      },
    ],
  };
}

describe("DefaultNodeLayout projection authority", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("renders projected title subtitle and pin names without resource stores", () => {
    act(() => root.render(<DefaultNodeLayout node={projectedNode()} />));

    expect(container.textContent).toContain("Sales Database");
    expect(container.textContent).toContain("Prior period");
    expect(container.querySelector('[data-testid="pin-name"]')?.textContent).toBe("amount");
  });

  it("renders inlineAndDetail parameters as read-only summaries even with graph context", () => {
    const node = projectedNode();
    node.parameterEditors = [
      {
        key: "value",
        display: { title: "Value", description: null },
        editor: "number",
        presentation: "inlineAndDetail",
        valueType: { kind: "Int64" },
        multiline: false,
        value: 42,
        configuration: null,
        inheritedValue: null,
        valueSource: null,
        options: null,
      },
    ];

    act(() => root.render(<DefaultNodeLayout node={node} graphPath="events/Main.yssbi-event" />));

    expect(container.textContent).toContain("Value");
    expect(container.textContent).toContain("42");
    expect(container.querySelector("input")).toBeNull();
    expect(container.querySelector('[role="switch"]')).toBeNull();
  });

  it("renders a projected inline value read-only without graphPath", () => {
    const node = projectedNode();
    node.parameterEditors = [
      {
        key: "value",
        display: { title: "Value", description: null },
        editor: "number",
        presentation: "inlineAndDetail",
        valueType: { kind: "Int64" },
        multiline: false,
        value: 42,
        configuration: null,
        inheritedValue: null,
        valueSource: null,
        options: null,
      },
    ];

    act(() => root.render(<DefaultNodeLayout node={node} />));

    expect(container.textContent).toContain("Value");
    expect(container.textContent).toContain("42");
  });
});
