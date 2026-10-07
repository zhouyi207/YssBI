import { afterEach, expect, it } from "vitest";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import {
  detailResource,
  remapDetailResource,
  resolveEditorDetailFocus,
} from "@/features/core/editor/detail/editorDetailPolicy";
import { clearDetailFocusForClosedPanel } from "./clearDetailFocusForClosedPanel";

afterEach(() => useEditorStore.getState().clearDetailFocus());

it("follows canvas selection and keeps ordinary resource details independent of selection", () => {
  const base = { resourceRef: "opaque-resource-id", panelInstanceId: "panel-a" };
  for (const resourceKind of ["event_graph", "function_graph"] as const) {
    const scope = { ...base, resourceKind };
    expect(resolveEditorDetailFocus(scope, ["node-a"])).toEqual({
      kind: "node",
      graphPath: base.resourceRef,
      id: "node-a",
    });
    expect(resolveEditorDetailFocus(scope, [])).toEqual({
      kind: resourceKind,
      path: base.resourceRef,
    });
    expect(resolveEditorDetailFocus(scope, ["node-a", "node-b"])).toEqual({
      kind: resourceKind,
      path: base.resourceRef,
    });
  }
  const mind = { ...base, resourceKind: "mind" as const };
  expect(resolveEditorDetailFocus(mind, ["topic-a"])).toEqual({
    kind: "mind",
    path: base.resourceRef,
    panelInstanceId: base.panelInstanceId,
    nodeId: "topic-a",
  });
  expect(resolveEditorDetailFocus(mind, [])).toMatchObject({ kind: "mind", nodeId: null });
  expect(resolveEditorDetailFocus(mind, ["topic-a", "topic-b"])).toMatchObject({
    kind: "mind",
    nodeId: null,
  });
  const fixed = [
    { resourceKind: "doc" as const, expected: { kind: "doc", path: base.resourceRef } },
    { resourceKind: "chart" as const, expected: { kind: "chart", chartPath: base.resourceRef } },
    { resourceKind: "database" as const, expected: { kind: "data", id: base.resourceRef } },
  ];
  for (const { resourceKind, expected } of fixed) {
    expect(resolveEditorDetailFocus({ ...base, resourceKind }, [])).toEqual(expected);
    expect(resolveEditorDetailFocus({ ...base, resourceKind }, ["unrelated-node"])).toEqual(
      expected,
    );
  }
});

it("tracks node, Doc and Mind resource identities through rename and scoped close", () => {
  const node = resolveEditorDetailFocus(
    {
      resourceKind: "function_graph",
      resourceRef: "functions/F.yssbi-function",
      panelInstanceId: "function-pane",
    },
    ["node-a"],
  );
  expect(
    detailResource(node, { file: { kind: "function_graph", id: "functions/F.yssbi-function" } }),
  ).toEqual({ kind: "function_graph", id: "functions/F.yssbi-function" });
  expect(detailResource(node, {})).toBeNull();
  const doc = resolveEditorDetailFocus(
    { resourceKind: "doc", resourceRef: "docs/Old.md", panelInstanceId: "doc-pane" },
    [],
  );
  const renamedDoc = remapDetailResource(doc, "docs/Old.md", "docs/New.md")!;
  expect(detailResource(renamedDoc)).toEqual({ kind: "doc", id: "docs/New.md" });
  useEditorStore.getState().setDetailFocus(renamedDoc);
  clearDetailFocusForClosedPanel("docs/Old.md");
  expect(useEditorStore.getState().detailFocus).toBe(renamedDoc);
  clearDetailFocusForClosedPanel("docs/New.md");
  expect(useEditorStore.getState().detailFocus).toBeNull();

  const mind = resolveEditorDetailFocus(
    { resourceKind: "mind", resourceRef: "minds/Old.yssbi-mind", panelInstanceId: "mind-pane" },
    ["topic-a"],
  );
  const renamedMind = remapDetailResource(mind, "minds/Old.yssbi-mind", "minds/New.yssbi-mind")!;
  expect(renamedMind).toMatchObject({
    nodeId: "topic-a",
    panelInstanceId: "mind-pane",
    path: "minds/New.yssbi-mind",
  });
  useEditorStore.getState().setDetailFocus(renamedMind);
  clearDetailFocusForClosedPanel("minds/New.yssbi-mind", "another-pane");
  expect(useEditorStore.getState().detailFocus).toBe(renamedMind);
  clearDetailFocusForClosedPanel("minds/New.yssbi-mind", "mind-pane");
  expect(useEditorStore.getState().detailFocus).toBeNull();
});
