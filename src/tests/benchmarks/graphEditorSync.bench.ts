import { parseEditorGraphProjectionDto } from "@/shared/types/domain/editorProjectionParser";
import { makeGraphEditorSession } from "@/tests/helpers/editorProjectionFixtures";
import { bench, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import projectionFixture from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import { invokeGraphSync, clearGraphSyncBaselines } from "@/services/nodeSystem/graphEditorSync";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const cases = new Map<
  string,
  { wire: string; nodeId: string; revision: number; snapshotBytes: number }
>();

vi.mocked(invoke).mockImplementation(async (_command, raw) => {
  const args = raw as {
    projectInstanceId: string;
    graphPath: string;
    locale: string;
    cursor: string | null;
  };
  const fixture = cases.get(args.projectInstanceId)!;
  if (!args.cursor) {
    const snapshot = JSON.parse(fixture.wire);
    snapshot.update.data.editing.version.revision = String(fixture.revision);
    return snapshot;
  }
  fixture.revision++;
  const x = fixture.revision % 2;
  return JSON.parse(
    JSON.stringify({
      projectInstanceId: args.projectInstanceId,
      graphPath: args.graphPath,
      locale: args.locale,
      changed: true,
      resourceRevision: null,
      functionEditorProjection: null,
      update: {
        kind: "delta",
        baseCursor: args.cursor,
        cursor: String(fixture.revision),
        snapshotBytes: fixture.snapshotBytes,
        changes: [
          { kind: "set", path: ["document", "nodes", fixture.nodeId, "position", "x"], value: x },
          { kind: "set", path: ["projection", "nodes", "0", "position", "x"], value: x },
          {
            kind: "set",
            path: ["editing", "version", "revision"],
            value: String(fixture.revision),
          },
          { kind: "set", path: ["editing", "dirty"], value: true },
          { kind: "set", path: ["editing", "canUndo"], value: true },
        ],
      },
    }),
  );
});

for (const count of [100, 1000, 5000]) {
  const binding = {
    projectInstanceId: `bench-${count}`,
    graphPath: `events/Bench-${count}.yssbi-event`,
    locale: "en-US",
  };
  const projection = structuredClone(projectionFixture);
  projection.graphPath = binding.graphPath;
  projection.basis.graphPath = binding.graphPath;
  const template = JSON.stringify(projection.nodes[0]);
  const templateId = projection.nodes[0].nodeId;
  projection.nodes = [];
  const document = {
    nodes: {} as Record<string, unknown>,
    port_bindings: [],
    connections: {},
    input_states: [],
  };
  let firstId = "";
  for (let index = 0; index < count; index++) {
    const id = `00000000-0000-0000-0000-${(index + 100).toString(16).padStart(12, "0")}`;
    if (!firstId) firstId = id;
    const node = JSON.parse(template.split(templateId).join(id));
    node.graphPath = binding.graphPath;
    node.position = { x: 0, y: index * 100 };
    projection.nodes.push(node);
    document.nodes[id] = {
      id,
      node_type: node.nodeTypeId,
      position: node.position,
      parameters: {},
      user_label: null,
    };
  }
  const response = {
    ...binding,
    changed: false,
    resourceRevision: null,
    functionEditorProjection: null,
    update: {
      kind: "snapshot",
      cursor: "initial",
      snapshotBytes: 0,
      data: {
        resultState: makeGraphEditorSession(parseEditorGraphProjectionDto(projection)).resultState,

        document,
        projection,
        editing: {
          version: { sessionId: "00000000-0000-0000-0000-000000000001", revision: "0" },
          dirty: false,
          canUndo: false,
          canRedo: false,
        },
      },
    },
  };
  response.update.snapshotBytes = Buffer.byteLength(JSON.stringify(response.update.data));
  const wire = JSON.stringify(response);
  cases.set(binding.projectInstanceId, {
    wire,
    nodeId: firstId,
    revision: 0,
    snapshotBytes: response.update.snapshotBytes,
  });
  const install = async () => {
    const reply = await invokeGraphSync("hydrate_editor_graph", binding, binding);
    useGraphProjectionStore.getState().hydrate(binding.graphPath, reply.data);
  };
  bench(
    `${count} nodes: snapshot parse and adoption`,
    async () => {
      clearGraphSyncBaselines();
      useGraphProjectionStore.getState().clearGraph(binding.graphPath);
      await install();
    },
    { time: 500, iterations: 30, warmupTime: 100 },
  );
  bench(`${count} nodes: delta parse and adoption`, install, {
    time: 500,
    iterations: 30,
    warmupTime: 100,
  });
}
