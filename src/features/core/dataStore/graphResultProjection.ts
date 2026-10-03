import { shallow } from "zustand/shallow";
import { graphOutputKey, portAddressKey } from "@/features/domain/editorProjection";
import type { GraphResultState } from "@/shared/types/domain/result";

/** Share validated result rows by their addresses at the existing publication boundary. */
export function shareGraphResultState(
  previous: GraphResultState | undefined,
  next: GraphResultState,
): GraphResultState {
  if (!previous || previous === next) return next;
  let outputs = next.outputs;
  if (outputs !== previous.outputs) {
    const byOutput = new Map(
      previous.outputs.map((entry) => [graphOutputKey(entry.output), entry]),
    );
    const shared = outputs.map((entry) => {
      const before = byOutput.get(graphOutputKey(entry.output));
      if (!before || before === entry) return entry;
      if (before.state === entry.state && before.resultId === entry.resultId) return before;
      return before.output === entry.output ? entry : { ...entry, output: before.output };
    });
    outputs = shallow(previous.outputs, shared)
      ? previous.outputs
      : shallow(outputs, shared)
        ? outputs
        : shared;
  }
  let connections = next.connections;
  if (connections !== previous.connections) {
    const key = (entry: GraphResultState["connections"][number]) =>
      JSON.stringify([graphOutputKey(entry.output), portAddressKey(entry.input)]);
    const byEndpoints = new Map(previous.connections.map((entry) => [key(entry), entry]));
    const shared = connections.map((entry) => {
      const before = byEndpoints.get(key(entry));
      if (!before || before === entry) return entry;
      if (before.state === entry.state) return before;
      return before.output === entry.output && before.input === entry.input
        ? entry
        : { ...entry, output: before.output, input: before.input };
    });
    connections = shallow(previous.connections, shared)
      ? previous.connections
      : shallow(connections, shared)
        ? connections
        : shared;
  }
  const shared =
    outputs === next.outputs && connections === next.connections
      ? next
      : { ...next, outputs, connections };
  return shallow(previous, shared) ? previous : shared;
}
