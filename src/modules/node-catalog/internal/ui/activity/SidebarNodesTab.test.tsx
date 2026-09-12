// @vitest-environment happy-dom
import { act } from "react";
import { useActivityPanelExpansion } from "@/features/application/sidebar/useActivityPanelExpansion";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import type { NodeCreationDescriptorDto } from "@/shared/types/domain/nodeCreationDescriptor";

const draggableInputs = vi.hoisted(() => [] as Array<{ id: string; data: unknown }>);
vi.mock("@dnd-kit/core", () => ({
  useDraggable: (input: { id: string; data: unknown }) => {
    draggableInputs.push(input);
    return { attributes: {}, listeners: {}, setNodeRef: vi.fn() };
  },
}));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@/features/application/sidebar/useActivityPanelDocument", () => ({
  useActivityPanelDocument: () => ({
    document: backendDocument,
    error: null,
    refresh: vi.fn(),
    ...useActivityPanelExpansion("nodes"),
  }),
}));
import { SidebarNodesTab } from "./SidebarNodesTab";
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const helperCreation: NodeCreationDescriptorDto = {
  kind: "resourceBound",
  nodeTypeId: "function.call",
  resourcePath: "functions/Helper",
  resourceRevision: 1,
  createArgs: { kind: "function" },
};
const otherCreation: NodeCreationDescriptorDto = {
  ...helperCreation,
  resourcePath: "functions/Other",
};
const backendDocument = activityPanelFixture("nodes", [
  categoryFixture("statistics", "Statistics"),
  categoryFixture("statistics.regression", "Regression", 1),
  {
    kind: "item",
    id: "node:logit",
    depth: 2,
    item: {
      kind: "node",
      key: "static:statistics.logit.fit",
      title: "Logit fit",
      creation: { kind: "static", nodeTypeId: "statistics.logit.fit" },
    },
  },
  categoryFixture("output", "Output"),
  {
    kind: "item",
    id: "node:helper",
    depth: 1,
    item: {
      kind: "node",
      key: "resourceBound:function.call:Helper",
      title: "Call Helper",
      creation: helperCreation,
    },
  },
  {
    kind: "item",
    id: "node:other",
    depth: 1,
    item: {
      kind: "node",
      key: "resourceBound:function.call:Other",
      title: "Call Other",
      creation: otherCreation,
    },
  },
]);
describe("SidebarNodesTab", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useSidebarStore.setState({ expandedCategories: {} });
    draggableInputs.length = 0;

    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("keeps resource-bound entries distinct while preserving their descriptors", () => {
    act(() =>
      root.render(
        <TooltipProvider>
          <SidebarNodesTab />
        </TooltipProvider>,
      ),
    );
    const output = host.querySelector<HTMLButtonElement>(
      '[data-sidebar-tree-category-id="output"]',
    )!;
    act(() => output.click());

    const nodeInputs = draggableInputs.filter(
      ({ data }) => (data as { type?: string }).type === "node-template",
    );
    expect(nodeInputs).toHaveLength(2);
    expect(new Set(nodeInputs.map(({ id }) => id)).size).toBe(2);
    expect(
      nodeInputs.map(
        ({ data }) =>
          (
            data as {
              template: { descriptor: unknown };
            }
          ).template.descriptor,
      ),
    ).toEqual([helperCreation, otherCreation]);
  });
});
