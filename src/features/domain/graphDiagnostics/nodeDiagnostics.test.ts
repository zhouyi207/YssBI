import { describe, expect, it } from "vitest";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { DiagnosticDto } from "@/shared/types/dto/editorProjection";
import {
  collectGraphProblems,
  findPrimaryPortDiagnostic,
  isUnboundInputDiagnostic,
  type GraphProblemsBucket,
} from "./nodeDiagnostics";

const diagnostic = (
  code: string,
  message: string,
  nodeId = "unused-in-fixture",
): DiagnosticDto => ({
  code,
  messageKey: `diagnostics.${code}`,
  arguments: { value: message },
  severity: code === "error" ? "error" : "warning",
  blocking: code === "error",
  location: { kind: "node", nodeId },
  related: [],
});

const bucket = {
  graphNodes: ["node-a", "node-b"],
  nodes: {
    "node-a": {
      id: "node-a",
      display: { title: "Node A" },
      diagnostics: [
        diagnostic("error", "A failed", "node-a"),
        diagnostic("warning", "A needs review", "node-a"),
      ],
    },
    "node-b": {
      id: "node-b",
      display: { title: "Node B" },
      diagnostics: [diagnostic("warning", "B needs review", "node-b")],
    },
  },
  diagnostics: [
    diagnostic("error", "A failed", "node-a"),
    diagnostic("warning", "A needs review", "node-a"),
    diagnostic("warning", "B needs review", "node-b"),
  ],
} as unknown as GraphProblemsBucket;

describe("collectGraphProblems", () => {
  it("collects canonical graph diagnostics with their resolved locations", () => {
    expect(collectGraphProblems("events/Main.yssbi-event", bucket)).toEqual([
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: "node-a",
        locationLabel: "Node A",
        diagnostic: diagnostic("error", "A failed", "node-a"),
      },
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: "node-a",
        locationLabel: "Node A",
        diagnostic: diagnostic("warning", "A needs review", "node-a"),
      },
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: "node-b",
        locationLabel: "Node B",
        diagnostic: diagnostic("warning", "B needs review", "node-b"),
      },
    ]);
  });

  it("returns no rows when the graph projection is unavailable", () => {
    expect(collectGraphProblems("events/Main.yssbi-event", undefined)).toEqual([]);
  });

  it("formats port locations with projected node and pin titles", () => {
    const address = { kind: "declared" as const, nodeId: "node-a", portKey: "value" };
    const portDiagnostic: DiagnosticDto = {
      ...diagnostic("error", "Value is invalid", "node-a"),
      location: { kind: "port", address },
    };
    const pinId = portAddressKey(address);
    const portBucket = {
      graphNodes: ["node-a"],
      nodes: {
        "node-a": {
          id: "node-a",
          display: { title: "Node A" },
          diagnostics: [portDiagnostic],
        },
      },
      pins: {
        [pinId]: {
          id: pinId,
          nodeId: "node-a",
          name: "raw-value",
          display: { label: "Value", instanceLabel: null },
          address,
        },
      },
      diagnostics: [portDiagnostic],
    } as unknown as GraphProblemsBucket;

    expect(collectGraphProblems("events/Main.yssbi-event", portBucket)).toEqual([
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: "node-a",
        locationLabel: "Node A · Value",
        diagnostic: portDiagnostic,
      },
    ]);
  });

  it("includes graph, resource, connection, and parameter problems absent from node indexes", () => {
    const output = { kind: "declared" as const, nodeId: "node-a", portKey: "value" };
    const input = { kind: "declared" as const, nodeId: "node-b", portKey: "input" };
    const graphProblem: DiagnosticDto = {
      ...diagnostic("warning", "Graph needs review"),
      location: { kind: "graph" },
    };
    const resourceProblem: DiagnosticDto = {
      ...diagnostic("warning", "Resource is unavailable"),
      location: { kind: "resource", identity: "database/source" },
    };
    const connectionProblem: DiagnosticDto = {
      ...diagnostic("error", "Connection is invalid"),
      location: { kind: "connection", connectionId: "connection-1" },
    };
    const parameterProblem: DiagnosticDto = {
      ...diagnostic("error", "Parameter is invalid", "node-a"),
      location: { kind: "parameter", nodeId: "node-a", key: "threshold" },
    };
    const completeBucket = {
      graphNodes: ["node-a", "node-b"],
      nodes: {
        "node-a": {
          display: { title: "Node A" },
          diagnostics: [],
          parameterEditors: [{ key: "threshold", display: { title: "Threshold" } }],
        },
        "node-b": { display: { title: "Node B" }, diagnostics: [], parameterEditors: [] },
      },
      pins: {
        [portAddressKey(output)]: { name: "value", display: { label: "Value" } },
        [portAddressKey(input)]: { name: "input", display: { label: "Input" } },
      },
      connections: { "connection-1": { output, input } },
      diagnostics: [graphProblem, resourceProblem, connectionProblem, parameterProblem],
    } as unknown as GraphProblemsBucket;

    expect(collectGraphProblems("events/Main.yssbi-event", completeBucket)).toEqual([
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: null,
        locationLabel: "events/Main.yssbi-event",
        diagnostic: graphProblem,
      },
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: null,
        locationLabel: "database/source",
        diagnostic: resourceProblem,
      },
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: null,
        locationLabel: "Node A · Value → Node B · Input",
        diagnostic: connectionProblem,
      },
      {
        graphPath: "events/Main.yssbi-event",
        nodeId: "node-a",
        locationLabel: "Node A · Threshold",
        diagnostic: parameterProblem,
      },
    ]);
  });

  it("uses the projected port diagnostic to identify an unbound input", () => {
    const address = { kind: "declared" as const, nodeId: "node-a", portKey: "value" };
    const unrelated = diagnostic("warning", "Other node", "node-b");
    const unbound: DiagnosticDto = {
      ...diagnostic("error", "Required input is unbound", "node-a"),
      code: "compiler.input.unbound",
      location: { kind: "port", address },
    };

    const selected = findPrimaryPortDiagnostic([unrelated, unbound], address);
    expect(selected).toBe(unbound);
    expect(isUnboundInputDiagnostic(selected)).toBe(true);
  });
});
