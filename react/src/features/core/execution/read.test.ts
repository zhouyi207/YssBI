import { afterEach, expect, it, vi } from "vitest";
import * as readProjection from "@/features/core/state/readProjection";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { useExecutionRead } from "./read";
import { useExecutionStore } from "./useExecutionStore";

afterEach(() => {
  vi.restoreAllMocks();
  useExecutionStore.setState({ graphs: {} });
  useResourceStore.getState().clear();
});

it("keeps output-only changes out of execution views and hides failures after semantic changes", () => {
  vi.spyOn(readProjection, "useReadProjection").mockImplementation((projection, select) =>
    select(projection.getSnapshot()),
  );
  const read = () => useExecutionRead((snapshot) => snapshot);
  const graphPath = "events/Main.yssbi-event";
  const otherPath = "events/Other.yssbi-event";
  const fixture = makeEditorProjectionFixture({ graphPath });
  const session = makeGraphEditorSession(fixture.projection);
  useResourceStore.getState().installGraphSession(graphPath, session);
  const run = {
    graphPath,
    runId: "1",
    executionSessionId: session.resultState.executionSessionId,
    semanticInputHash: fixture.projection.basis.semanticInputHash,
  };
  const store = useExecutionStore.getState();
  store.markExecutionUnknown(otherPath);
  store.applyRunEvent({ resultRevision: "1", run, kind: { type: "runStarted", outputs: [] } });
  store.applyRunEvent({
    resultRevision: "1",
    run,
    kind: {
      type: "runErrored",
      groups: [
        {
          caller: { graphPath, nodeId: null, portAddress: null },
          function: "functions/Group.yssbi-function",
          ordinal: "2",
        },
      ],
      code: "groupSchemaMismatch",
      phase: "execution",
      source: null,
    },
  });
  const failed = read();
  expect(failed.graphs[graphPath].runFailure).toBe(store.getGraph(graphPath).runFailure);
  expect(failed.graphs[graphPath].runFailure?.groups[0].ordinal).toBe("2");
  store.applyRunEvent(
    {
      resultRevision: "2",
      run: { ...run, runId: "2" },
      kind: { type: "runStarted", outputs: [{ graphPath, port: fixture.outputAddress }] },
    },
    false,
  );
  expect(read()).toBe(failed);

  const changed = structuredClone(fixture.projection);
  changed.basis.semanticInputHash = "2".repeat(64);
  useResourceStore.getState().installGraphSession(graphPath, makeGraphEditorSession(changed));
  const current = read();
  expect(current.graphs[graphPath]).toMatchObject({
    status: "error",
    runId: null,
    runFailure: null,
  });
  expect(current.graphs[otherPath]).toBe(failed.graphs[otherPath]);
  expect(failed.graphs[graphPath].runFailure?.code).toBe("groupSchemaMismatch");

  store.releaseGraphExecutionState(graphPath);
  expect(read().graphs[graphPath]).toBeUndefined();
  expect(read().graphs[otherPath]).toBe(failed.graphs[otherPath]);
  expect(current.graphs[graphPath].status).toBe("error");
});
