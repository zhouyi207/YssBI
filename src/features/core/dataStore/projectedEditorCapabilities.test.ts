import {
  installGraphProjectionFixture,
  makeEditorProjectionFixture,
} from "@/tests/helpers/editorProjectionFixtures";
import { beforeEach, describe, expect, it } from "vitest";

import { useGraphProjectionStore } from "./graphProjectionStore";
import { isUnmanagedNode } from "./graphNodeSelectors";

const graphPath = "functions/projected-capabilities";
const nodeId = "managed-node";

describe("projected node ownership", () => {
  beforeEach(() => useGraphProjectionStore.getState().clear());

  it("does not authorize operations without a node projection", () => {
    expect(isUnmanagedNode(graphPath, nodeId)).toBe(false);
  });

  it.each([true, false])(
    "uses the Rust managed flag (%s) for graph content operations",
    (managed) => {
      const fixture = makeEditorProjectionFixture({ graphPath, nodeId });
      fixture.projection.nodes[0].capabilities = { managed };
      installGraphProjectionFixture(graphPath, fixture.projection);
      expect(isUnmanagedNode(graphPath, nodeId)).toBe(!managed);
    },
  );
});
