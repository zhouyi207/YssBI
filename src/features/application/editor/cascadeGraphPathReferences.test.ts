import { beforeEach, describe, expect, it } from "vitest";
import { useGraphProjectionStore } from "@/features/core/dataStore";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { remapGraphNonViewportUiState } from "./cascadeGraphPathReferences";

const from = "functions/Old.yssbi-function";
const to = "functions/New.yssbi-function";

describe("remapGraphNonViewportUiState", () => {
  beforeEach(() => {
    useGraphProjectionStore.setState({ graphEntities: {} });
    useGraphMetaStore.setState({
      graphs: { [from]: { path: from, name: "Old", type: "function" } },
    });
    useEditorStore.setState({
      detailFocus: { kind: "function", path: from },
    });
    const fixture = makeEditorProjectionFixture({
      graphPath: "events/Caller.yssbi-event",
      nodeId: "call-1",
      nodeTypeId: "yssbi.project.function.call",
    });
    useGraphProjectionStore
      .getState()
      .replaceProjection("events/Caller.yssbi-event", fixture.projection);
  });

  it("remaps editor focus and selection without mutating domain projections", () => {
    const graphBefore = structuredClone(useGraphProjectionStore.getState().graphEntities);
    const metaBefore = structuredClone(useGraphMetaStore.getState().graphs);

    remapGraphNonViewportUiState(from, to);

    expect(useEditorStore.getState().detailFocus).toEqual({ kind: "function", path: to });
    expect(useGraphProjectionStore.getState().graphEntities).toEqual(graphBefore);
    expect(useGraphMetaStore.getState().graphs).toEqual(metaBefore);
  });
});
