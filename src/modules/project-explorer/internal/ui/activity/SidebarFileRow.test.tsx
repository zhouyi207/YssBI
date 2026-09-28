// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DRAG_TYPES } from "@/features/core/dnd";

const mocks = vi.hoisted(() => ({
  listItemProps: [] as Array<Record<string, unknown>>,
  openFileInEditor: vi.fn(),
  revealDetails: vi.fn(),
}));

vi.mock("@/modules/workbench/public", () => ({
  SidebarListItem: (props: Record<string, unknown>) => {
    mocks.listItemProps.push(props);
    return <div>{String(props.label)}</div>;
  },
  SidebarRowActionButton: () => null,
  SIDEBAR_ROW_ICON_SIZE: 16,
}));
vi.mock("@/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => children,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => children,
  TooltipContent: ({ children }: { children: React.ReactNode }) => children,
}));
vi.mock("@/features/application/editor/openFileInEditor", () => ({
  openFileInEditor: mocks.openFileInEditor,
}));
vi.mock("@/features/application/editor/rightSidebarActions", () => ({
  revealDetails: mocks.revealDetails,
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { SidebarFileRow } from "./SidebarFileRow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("SidebarFileRow", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listItemProps.length = 0;
    mocks.revealDetails.mockResolvedValue(undefined);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("passes the exact Function graph-resource payload to SidebarListItem", () => {
    const functionResource = {
      id: "functions/Revenue.yssbi-function",
      name: "Revenue",
      type: "function_graph" as const,
    };

    act(() =>
      root.render(
        <SidebarFileRow
          id={functionResource.id}
          name={functionResource.name}
          kind={functionResource.type}
          onContextMenu={vi.fn()}
        />,
      ),
    );

    expect(host.textContent).toBe("Revenue");
    expect(mocks.listItemProps).toHaveLength(1);
    expect(mocks.listItemProps[0]?.dragData).toEqual({
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: functionResource,
    });
  });

  it("explicitly reveals graph Details before completing the row click action", async () => {
    act(() =>
      root.render(
        <SidebarFileRow
          id="events/Main.yssbi-event"
          name="Main"
          kind="event_graph"
          onContextMenu={vi.fn()}
        />,
      ),
    );
    const onClick = mocks.listItemProps[0]?.onClick as
      | ((event: { stopPropagation(): void }) => Promise<void>)
      | undefined;
    const stopPropagation = vi.fn();

    await act(async () => {
      await onClick?.({ stopPropagation });
    });

    expect(stopPropagation).toHaveBeenCalledOnce();
    expect(mocks.revealDetails).toHaveBeenCalledWith({
      kind: "event_graph",
      path: "events/Main.yssbi-event",
    });
    expect(mocks.openFileInEditor).toHaveBeenCalledWith("events/Main.yssbi-event", "event_graph");
  });
});
