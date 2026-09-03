import { beforeEach, describe, expect, it } from "vitest";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { useGraphProjectionStore } from "./graphProjectionStore";
import { canCopyNode, canDeleteNode } from "./graphNodeSelectors";

const graphPath = "functions/projected-capabilities";
const nodeId = "managed-node";

function installClipboardNode(canCopy: boolean | undefined, managed: boolean) {
  const fixture = makeEditorProjectionFixture({ graphPath, nodeId });
  const capabilities = {
    ...fixture.projection.nodes[0].capabilities,
    managed,
    canCopy: canCopy ?? true,
  };
  if (canCopy === undefined) {
    delete (capabilities as { canCopy?: boolean }).canCopy;
  }
  fixture.projection.nodes[0].capabilities = capabilities;
  useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection, 1);
}

function installProjectedCapabilities() {
  const fixture = makeEditorProjectionFixture({ graphPath, nodeId });
  const node = fixture.projection.nodes[0];
  node.capabilities = {
    ...node.capabilities,
    managed: true,
    canCopy: false,
    canDelete: false,
  };
  useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection, 1);
}

describe("projected active-editor capabilities", () => {
  beforeEach(() => {
    useGraphProjectionStore.setState({ graphEntities: {} });
  });

  it("protects Rust-managed nodes without a frontend node registry", () => {
    installProjectedCapabilities();

    expect(canDeleteNode(graphPath, nodeId)).toBe(false);
  });

  it("does not copy a node whose Rust projection disables copying", () => {
    installProjectedCapabilities();

    expect(canCopyNode(graphPath, nodeId)).toBe(false);
  });

  it.each([
    { label: "claims it is copyable", canCopy: true },
    { label: "omits canCopy", canCopy: undefined },
  ])("gives managed capability precedence when a node $label", ({ canCopy }) => {
    installClipboardNode(canCopy, true);

    expect(canCopyNode(graphPath, nodeId)).toBe(false);
  });

  it("allows copying an ordinary projected node", () => {
    installClipboardNode(true, false);

    expect(canCopyNode(graphPath, nodeId)).toBe(true);
  });
});
