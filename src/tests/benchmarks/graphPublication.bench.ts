import { bench } from "vitest";
import {
  prepareGraphSessions,
  type GraphProjectionData,
} from "@/features/core/dataStore/graphProjection";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useExecutionStore } from "@/features/core/execution";
import type { RunFailureProjection } from "@/features/core/execution/executionTypes";
import { graphOutputKey } from "@/features/domain/editorProjection";
import { freezePublishedValue } from "@/shared/types/deepReadonly";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
// Include the real read projection and its subscriptions in execution-update timings.
import "@/features/application/results/runtime";

function sessionFixture(graphPath: string, count: number) {
  const parts = Array.from({ length: count }, (_, index) =>
    makeEditorProjectionFixture({
      graphPath,
      nodeId: `00000000-0000-0000-0000-${(index + 1).toString(16).padStart(12, "0")}`,
    }),
  );
  const connections = parts.slice(1).map((part, index) => ({
    connectionId: `connection-${index}`,
    output: parts[index].outputAddress,
    input: part.inputAddress,
    order: null,
  }));
  const session = makeGraphEditorSession({
    ...parts[0].projection,
    nodes: parts.flatMap((part) => part.projection.nodes),
    connections,
  });
  session.resultState = {
    ...session.resultState,
    outputs: session.resultState.outputs.map((entry, index) => ({
      ...entry,
      state: "valid",
      resultId: String(index + 1),
    })),
    connections: connections.map(({ output, input }) => ({
      output: { graphPath, port: output },
      input,
      state: "valid",
    })),
  };
  freezePublishedValue(session);
  return session;
}

for (const count of [100, 500]) {
  const replacements = Array.from({ length: count }, (_, index) => {
    const graphPath = `events/Batch-${index}.yssbi-event`;
    return { graphPath, session: sessionFixture(graphPath, 10) };
  });
  const base: GraphProjectionData = { sessions: {}, graphEntities: {}, resultStates: {} };
  freezePublishedValue(base);
  bench(
    `${count} graphs: batch preparation`,
    () => {
      prepareGraphSessions(replacements, undefined, base);
    },
    {
      time: 500,
      iterations: 30,
      warmupTime: 100,
    },
  );
}

for (const count of [1000, 5000]) {
  const graphPath = `events/Presentation-${count}.yssbi-event`;
  const session = sessionFixture(graphPath, count);
  const failure: RunFailureProjection = {
    run: {
      graphPath,
      runId: "1",
      executionSessionId: session.resultState.executionSessionId,
      semanticInputHash: session.projection.basis.semanticInputHash,
    },
    code: "kernelFailed",
    phase: "execution",
    source: { graphPath, nodeId: session.projection.nodes[0].nodeId, portAddress: null },
    incidentId: null,
  };
  const outputRuns = Object.fromEntries(
    session.resultState.outputs.map(({ output }) => [
      graphOutputKey(output),
      {
        run: failure.run,
        output,
        active: false,
        resultRevision: session.resultState.revision,
      },
    ]),
  );
  freezePublishedValue(outputRuns);
  let failed = false;
  bench(
    `${count} outputs and run records: failure presentation update`,
    () => {
      const execution = useExecutionStore.getState();
      failed = !failed;
      useExecutionStore.setState({
        graphs: {
          ...execution.graphs,
          [graphPath]: {
            ...execution.graphs[graphPath],
            status: failed ? "error" : "idle",
            run: null,
            request: null,
            runFailure: failed ? failure : null,
          },
        },
      });
    },
    {
      time: 500,
      iterations: 30,
      warmupTime: 100,
      setup: () => {
        useResourceStore.getState().clear();
        useExecutionStore.setState({
          graphs: {
            [graphPath]: {
              status: "idle",
              run: null,
              request: null,
              runFailure: null,
              outputRuns,
            },
          },
        });
        useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
        failed = false;
      },
    },
  );
}
