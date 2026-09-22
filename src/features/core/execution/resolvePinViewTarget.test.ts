import { describe, expect, it } from "vitest";
import type {
  EditorConnectionProjectionDto,
  PortAddressDto,
} from "@/shared/types/dto/editorProjection";
import { inspectableRefsFromPinView, resolveUpstreamOutputs } from "./pinViewTarget";

const graphPath = "events/Main.yssbi-event";
const output: PortAddressDto = {
  kind: "declared",
  nodeId: "node-out",
  portKey: "result",
};
const input: PortAddressDto = {
  kind: "declared",
  nodeId: "node-in",
  portKey: "data",
};
const connection: EditorConnectionProjectionDto = {
  connectionId: "connection-1",
  output,
  input,
  order: null,
};

describe("pinViewTarget", () => {
  it("builds an authoritative output-pin result ref before any run", () => {
    const refs = inspectableRefsFromPinView({
      graphPath,
      address: output,
      direction: "output",
    });

    expect(refs).toEqual([{ kind: "outputPin", graphPath, output }]);
  });

  it("resolves an input only to its connected upstream output address", () => {
    const refs = inspectableRefsFromPinView({
      graphPath,
      address: input,
      direction: "input",
      connections: [connection],
    });

    expect(resolveUpstreamOutputs(input, [connection])).toEqual([output]);
    expect(refs).toEqual([{ kind: "outputPin", graphPath, output }]);
  });

  it("never creates input result when the connection does not target that input", () => {
    const otherInput: PortAddressDto = { ...input, portKey: "other" };
    expect(
      inspectableRefsFromPinView({
        graphPath,
        address: otherInput,
        direction: "input",
        connections: [connection],
      }),
    ).toEqual([]);
  });

  it("has no result reference for unconnected input pins", () => {
    expect(
      inspectableRefsFromPinView({
        graphPath,
        address: input,
        direction: "input",
        connections: [],
      }),
    ).toEqual([]);
  });
});
