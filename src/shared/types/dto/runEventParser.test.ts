import { describe, expect, it } from "vitest";
import executionWire from "@/tests/fixtures/node-system-contracts/execution-wire.json";
import { EXECUTION_DEMAND_TYPES, type ExecutionDemandDto } from "./executionDemand";
import { RUN_EVENT_KIND_TYPES } from "./runEvent";
import { parseExecutionDemandDto, parseRunEvent } from "./runEventParser";

function clone(value: unknown): unknown {
  return structuredClone(value);
}

function record(value: unknown): Record<string, unknown> {
  return value as Record<string, unknown>;
}

describe("execution wire parsers", () => {
  it("parses every Rust-generated execution demand variant", () => {
    expect(executionWire.demands.map(parseExecutionDemandDto)).toEqual(executionWire.demands);
    expect(executionWire.demands.map((demand) => demand.type)).toEqual(
      Object.keys(EXECUTION_DEMAND_TYPES),
    );
    expect(() =>
      parseExecutionDemandDto({
        type: "outputs",
        outputs: [],
        includeDefaultResults: false,
      }),
    ).not.toThrow();
  });

  it.each(executionWire.demands)("rejects extra keys on demand $type", (valid) => {
    expect(() => parseExecutionDemandDto({ ...valid, extra: true })).toThrow();
  });

  it("strictly validates output references and both port-address variants", () => {
    const outputs = executionWire.demands.find((demand) => demand.type === "outputs");
    if (!outputs?.outputs) throw new Error("missing outputs fixture");
    expect(outputs.outputs.map((output) => output.port.kind)).toEqual(["declared", "instance"]);

    const extraOutput = clone(outputs);
    Object.assign((record(extraOutput).outputs as Array<Record<string, unknown>>)[0], {
      extra: true,
    });
    expect(() => parseExecutionDemandDto(extraOutput)).toThrow();

    const extraPort = clone(outputs);
    const firstOutput = (record(extraPort).outputs as Array<Record<string, unknown>>)[0];
    Object.assign(firstOutput.port as object, { extra: true });
    expect(() => parseExecutionDemandDto(extraPort)).toThrow();
  });

  it.each(["", null])("rejects malformed graph output path %j", (graphPath) => {
    const outputs = clone(executionWire.demands.find((demand) => demand.type === "outputs"));
    const firstOutput = (record(outputs).outputs as Array<Record<string, unknown>>)[0];
    firstOutput.graphPath = graphPath;

    expect(() => parseExecutionDemandDto(outputs)).toThrow("graph output reference");
  });

  it.each([
    "events/folder/sub-folder/Main.v2.yssbi-event",
    "functions/library/math/Calculate.yssbi-function",
    "events/Sales Report 中文.yssbi-event",
    "functions/销售 预测.yssbi-function",
  ])("accepts opaque execution graph path %j", (graphPath) => {
    const outputs = clone(executionWire.demands.find((demand) => demand.type === "outputs"));
    const firstOutput = (record(outputs).outputs as Array<Record<string, unknown>>)[0];
    firstOutput.graphPath = graphPath;
    expect(() => parseExecutionDemandDto(outputs)).not.toThrow();

    const event = executionWire.runEvents[0];
    expect(() =>
      parseRunEvent({
        ...event,
        run: { ...event.run, graphPath },
      }),
    ).not.toThrow();
  });

  it("requires UUID-backed declared and instance port identities", () => {
    const outputs = clone(executionWire.demands.find((demand) => demand.type === "outputs"));
    const references = record(outputs).outputs as Array<Record<string, unknown>>;

    (references[0].port as Record<string, unknown>).nodeId = "not-a-uuid";
    expect(() => parseExecutionDemandDto(outputs)).toThrow("graph output reference");

    (references[0].port as Record<string, unknown>).nodeId = "00000000-0000-0000-0000-000000000002";
    (references[1].port as Record<string, unknown>).nodeId = "not-a-uuid";
    expect(() => parseExecutionDemandDto(outputs)).toThrow("graph output reference");

    (references[1].port as Record<string, unknown>).nodeId = "00000000-0000-0000-0000-000000000002";
    (references[1].port as Record<string, unknown>).instanceId = "not-a-uuid";
    expect(() => parseExecutionDemandDto(outputs)).toThrow("graph output reference");
  });

  it("parses the exact minimal Rust-generated run-event inventory", () => {
    const valid = executionWire.runEvents[0];

    expect(executionWire.runEvents.map(parseRunEvent)).toEqual(executionWire.runEvents);
    expect([...new Set(executionWire.runEvents.map((event) => event.kind.type))]).toEqual(
      Object.keys(RUN_EVENT_KIND_TYPES),
    );

    expect(() =>
      parseRunEvent({
        correlation: {},
        basis: {},
        kind: { type: "runStarted", outputs: [] },
      }),
    ).toThrow("Invalid run event");

    expect(() =>
      parseRunEvent({
        ...valid,
        run: { ...valid.run, runId: null },
      }),
    ).toThrow("Invalid graph run identity");
  });

  it.each(executionWire.runEvents)("rejects extra keys on RunEvent $kind.type", (valid) => {
    expect(() => parseRunEvent({ ...valid, extra: true })).toThrow();
    expect(() => parseRunEvent({ ...valid, run: { ...valid.run, extra: true } })).toThrow();
    expect(() => parseRunEvent({ ...valid, kind: { ...valid.kind, extra: true } })).toThrow();
  });

  it("strictly parses typed deadline phases and rejects malformed timeout wire", () => {
    const valid = executionWire.runEvents[0];
    const deadline = {
      ...valid,
      kind: { type: "runErrored", code: "deadlineExceeded", phase: "admission", source: null },
    };

    expect(parseRunEvent(deadline)).toEqual(deadline);
    const missingPhase = record(clone(deadline.kind));
    delete missingPhase.phase;
    expect(() => parseRunEvent({ ...valid, kind: missingPhase })).toThrow();

    for (const phase of [null, 1, "unknown"]) {
      expect(() => parseRunEvent({ ...deadline, kind: { ...deadline.kind, phase } })).toThrow();
    }
  });

  it("rejects unknown variants and malformed graph run identities", () => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({ ...valid, kind: { type: "unknown" } })).toThrow();
    expect(() =>
      parseRunEvent({
        ...valid,
        run: { ...valid.run, runId: "01" },
      }),
    ).toThrow("graph run identity");
    expect(() =>
      parseRunEvent({
        ...valid,
        run: { ...valid.run, executionSessionId: "" },
      }),
    ).toThrow("graph run identity");
  });

  it.each(["", null])("rejects malformed graph run path %j", (graphPath) => {
    const valid = executionWire.runEvents[0];
    expect(() =>
      parseRunEvent({
        ...valid,
        run: { ...valid.run, graphPath },
      }),
    ).toThrow("graph run identity");
  });
});

// Caller-side type constraints complement the runtime parser checks above.
// These assertions are checked by `pnpm check:ts`.
// @ts-expect-error outputs demand requires outputs
const missingOutputs: ExecutionDemandDto = { type: "outputs", includeDefaultResults: false };

// @ts-expect-error outputs demand requires includeDefaultResults
const missingIncludeDefaults: ExecutionDemandDto = { type: "outputs", outputs: [] };

// @ts-expect-error demand tags are closed
const invalidTag: ExecutionDemandDto = { type: "unknown" };

// @ts-expect-error default demand has no extra fields
const extraDefaultField: ExecutionDemandDto = { type: "default", outputs: [] };

const extraOutputField: ExecutionDemandDto = {
  type: "outputs",
  outputs: [
    {
      graphPath: "events/Main.yssbi-event",
      port: {
        kind: "declared",
        nodeId: "00000000-0000-0000-0000-000000000001",
        portKey: "result",
      },
      // @ts-expect-error output identities have no extra fields
      extra: true,
    },
  ],
  includeDefaultResults: false,
};

void [missingOutputs, missingIncludeDefaults, invalidTag, extraDefaultField, extraOutputField];
