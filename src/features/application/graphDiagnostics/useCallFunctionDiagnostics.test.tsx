// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { buildGraphResourceMeta, useResourceStore } from "@/features/core/resource";
import { BUILTIN_NODE_TYPE_IDS } from "@/features/domain/nodeCatalog";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { useCallFunctionIssue } from "./useCallFunctionDiagnostics";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const callGraphPath = "events/Caller.yssbi-event";
const callNodeId = "call-1";
const nonCallGraphPath = "events/Regular.yssbi-event";
const nonCallNodeId = "regular-1";
const targetPath = "functions/Target.yssbi-function";

type ProbeState = {
  renders: number;
  issue: ReturnType<typeof useCallFunctionIssue>;
};

function IssueProbe({
  graphPath,
  nodeId,
  state,
}: {
  graphPath: string;
  nodeId: string;
  state: ProbeState;
}) {
  state.renders += 1;
  state.issue = useCallFunctionIssue(graphPath, nodeId);
  return null;
}

function installNode(
  graphPath: string,
  nodeId: string,
  nodeTypeId: string,
  subGraphPath: string,
): void {
  const fixture = makeEditorProjectionFixture({ graphPath, nodeId, nodeTypeId });
  useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
  useGraphDataStore.setState((state) => ({
    graphEntities: {
      ...state.graphEntities,
      [graphPath]: {
        ...state.graphEntities[graphPath],
        nodes: {
          ...state.graphEntities[graphPath].nodes,
          [nodeId]: {
            ...state.graphEntities[graphPath].nodes[nodeId],
            subGraphPath,
          },
        },
      },
    },
  }));
}

describe("useCallFunctionIssue resource subscriptions", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    useGraphDataStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
  });

  it("invalidates only the Call Function consumer when target availability changes", () => {
    installNode(callGraphPath, callNodeId, BUILTIN_NODE_TYPE_IDS.callFunction, targetPath);
    installNode(nonCallGraphPath, nonCallNodeId, "tests.regular-node", targetPath);
    useResourceStore
      .getState()
      .upsertResource(buildGraphResourceMeta("function", targetPath, "Target"));
    const callProbe: ProbeState = { renders: 0, issue: null };
    const nonCallProbe: ProbeState = { renders: 0, issue: null };

    act(() =>
      root.render(
        <>
          <IssueProbe graphPath={callGraphPath} nodeId={callNodeId} state={callProbe} />
          <IssueProbe graphPath={nonCallGraphPath} nodeId={nonCallNodeId} state={nonCallProbe} />
        </>,
      ),
    );

    expect(callProbe).toEqual({ renders: 1, issue: null });
    expect(nonCallProbe).toEqual({ renders: 1, issue: null });

    act(() => {
      useResourceStore
        .getState()
        .upsertResource(buildGraphResourceMeta("event", "events/Other.yssbi-event", "Other"));
      useResourceStore
        .getState()
        .patchResource(
          { kind: "function", id: targetPath },
          { loaded: true, name: "Renamed target" },
        );
    });

    expect(callProbe.renders).toBe(1);
    expect(nonCallProbe.renders).toBe(1);

    act(() => {
      useResourceStore
        .getState()
        .patchResource({ kind: "function", id: targetPath }, { exists: false });
    });

    expect(callProbe).toEqual({
      renders: 2,
      issue: {
        graphPath: callGraphPath,
        nodeId: callNodeId,
        kind: "missing_target",
        subGraphPath: targetPath,
      },
    });
    expect(nonCallProbe).toEqual({ renders: 1, issue: null });
  });
});
