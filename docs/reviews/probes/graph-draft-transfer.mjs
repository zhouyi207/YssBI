// Synthetic wire/clone probe for the architecture review; this does not run Tauri or Graph Resolve.
import { readFileSync } from "node:fs";
import { cpus } from "node:os";
import { performance } from "node:perf_hooks";

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../src/tests/fixtures/node-system-contracts/editor-projection.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const uuid = (index) => `00000000-0000-0000-0000-${index.toString(16).padStart(12, "0")}`;
const bytes = (value) => Buffer.byteLength(JSON.stringify(value), "utf8");
let sink;

function measure(action) {
  for (let index = 0; index < 8; index++) sink = action();
  const samples = [];
  for (let index = 0; index < 60; index++) {
    const start = performance.now();
    sink = action();
    samples.push(performance.now() - start);
  }
  samples.sort((left, right) => left - right);
  return {
    p50Ms: Number(samples[Math.ceil(samples.length * 0.5) - 1].toFixed(3)),
    p95Ms: Number(samples[Math.ceil(samples.length * 0.95) - 1].toFixed(3)),
  };
}

const results = [100, 1000, 5000].map((count) => {
  const document = { nodes: {}, port_bindings: [], connections: {}, input_states: [] };
  const projection = { ...fixture, nodes: [] };
  for (let index = 1; index <= count; index++) {
    const id = uuid(index);
    const position = { x: (index % 50) * 180, y: Math.floor(index / 50) * 100 };
    document.nodes[id] = {
      id,
      node_type: "yssbi.constant.get",
      position,
      parameters: { constant: "00000000-0000-0000-0000-000000000004" },
      user_label: null,
    };
    const node = JSON.parse(
      JSON.stringify(fixture.nodes[0]).replaceAll(fixture.nodes[0].nodeId, id),
    );
    projection.nodes.push({ ...node, position });
  }
  const saved = structuredClone(document);
  const changed = structuredClone(document);
  changed.nodes[uuid(1)].position.x += 10;
  const mutation = {
    type: "moveNodes",
    payload: { positions: [{ nodeId: uuid(1), position: changed.nodes[uuid(1)].position }] },
  };
  const currentRequest = {
    projectInstanceId: uuid(100000),
    graphPath: fixture.graphPath,
    locale: "en-US",
    document: changed,
    mutation,
  };
  // Proposed command envelope, deliberately labelled as a model rather than an implemented API.
  const proposedRequest = {
    projectInstanceId: uuid(100000),
    graphPath: fixture.graphPath,
    draftSessionId: uuid(100001),
    expectedDraftRevision: "42",
    requestId: uuid(100002),
    mutation,
  };
  const currentReply = { changed: true, document: changed, projection };
  return {
    nodes: count,
    currentRequestBytes: bytes(currentRequest),
    proposedRequestBytes: bytes(proposedRequest),
    documentBytes: bytes(document),
    currentReplyBytes: bytes(currentReply),
    fiftyDocumentsSerializedBytes: bytes(document) * 50,
    requestStringify: measure(() => JSON.stringify(currentRequest)),
    storeCloneAndCompare: measure(() => ({
      dirty: JSON.stringify(changed) !== JSON.stringify(saved),
      document: structuredClone(changed),
      projection: structuredClone(projection),
      historyDocument: structuredClone(document),
    })),
  };
});

console.log(
  JSON.stringify(
    {
      measuredAt: new Date().toISOString(),
      runtime: process.version,
      platform: `${process.platform}/${process.arch}`,
      cpu: cpus()[0]?.model,
      warmups: 8,
      samples: 60,
      scope:
        "Synthetic homogeneous wire payloads and JavaScript cloning; no graph validation, Rust work, Tauri IPC, React render, or disk I/O. Proposed request is not an implemented API. Serialized bytes are not heap usage.",
      results,
    },
    null,
    2,
  ),
);
void sink;
