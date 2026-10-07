import type { Node } from "@xyflow/react";
import { expect, it } from "vitest";
import type { MindDocument } from "@/shared/types/domain/mind";
import { createFlowNodeViewProjector } from "@/shared/ui/flowNodeViewProjector";
import { createMindMapProjector } from "./mindProjection";

const document: MindDocument = {
  rootId: "root",
  nodes: [
    { id: "root", parentId: null, content: "Root" },
    { id: "branch", parentId: "root", content: "Branch" },
    { id: "leaf", parentId: "branch", content: "Leaf" },
    { id: "other", parentId: "root", content: "Other" },
  ],
};

it("preserves unchanged layout branches and topology when a Mind label changes", () => {
  const project = createMindMapProjector();
  const first = project(document, new Set());
  const edited = {
    ...document,
    nodes: document.nodes.map((node) =>
      node.id === "leaf" ? { ...node, content: "Changed" } : node,
    ),
  };
  const next = project(edited, new Set());

  expect(next.nodes.map((node) => node.position)).toEqual(first.nodes.map((node) => node.position));
  expect(next.nodes[0]).toBe(first.nodes[0]);
  expect(next.nodes[1]).toBe(first.nodes[1]);
  expect(next.nodes[2]).not.toBe(first.nodes[2]);
  expect(next.nodes[2].data.label).toBe("Changed");
  expect(first.nodes[2].data.label).toBe("Leaf");
  expect(next.nodes[3]).toBe(first.nodes[3]);
  expect(next.edges).toBe(first.edges);
  expect(next.nodeIds).toBe(first.nodeIds);
  expect(project({ ...edited }, new Set())).toBe(next);
});

it("shares Mind selection and measurement views and releases collapsed node measurements", () => {
  const project = createMindMapProjector();
  const views = createFlowNodeViewProjector<Node>();
  const positions = new Map();
  const model = project(document, new Set());
  const first = views.project({ model, positions, selectedNodeIds: new Set(), interactive: true });
  const selectedIds = new Set(["other"]);
  const selected = views.project({
    model,
    positions,
    selectedNodeIds: selectedIds,
    interactive: true,
  });
  expect(selected[0]).toBe(first[0]);
  expect(selected[3]).toMatchObject({ selected: true });
  expect(selected[3]).not.toBe(first[3]);

  expect(
    views.updateMeasurements([
      { type: "dimensions", id: "leaf", dimensions: { width: 120, height: 60 } },
    ]),
  ).toBe(true);
  const measured = views.project({
    model,
    positions,
    selectedNodeIds: selectedIds,
    interactive: true,
  });
  expect(measured[0]).toBe(selected[0]);
  expect(measured[3]).toBe(selected[3]);
  expect(measured[2].measured).toEqual({ width: 120, height: 60 });
  expect(
    views.updateMeasurements([
      { type: "dimensions", id: "leaf", dimensions: { width: 120, height: 60 } },
    ]),
  ).toBe(false);

  const collapsed = project(document, new Set(["branch"]));
  views.project({ model: collapsed, positions, selectedNodeIds: selectedIds, interactive: true });
  expect(collapsed.nodeIds.has("leaf")).toBe(false);
  expect(collapsed.edges.some((edge) => edge.target === "leaf")).toBe(false);
  expect(
    views.updateMeasurements([
      { type: "dimensions", id: "leaf", dimensions: { width: 130, height: 60 } },
    ]),
  ).toBe(false);
  const expanded = views.project({
    model: project(document, new Set()),
    positions,
    selectedNodeIds: selectedIds,
    interactive: true,
  });
  expect(expanded.find((node) => node.id === "leaf")?.measured).toBeUndefined();
  expect(measured[2].measured).toEqual({ width: 120, height: 60 });
});
