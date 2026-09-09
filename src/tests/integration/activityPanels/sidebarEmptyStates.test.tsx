// @vitest-environment happy-dom
import { act } from "react";
import { useActivityPanelExpansion } from "@/features/application/sidebar/useActivityPanelExpansion";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import { IpcError } from "@/services/ipc/ipcError";
const historyAvailability = vi.hoisted(() => ({
  activeResourceRef: null as string | null,
  canUndo: false,
  canRedo: false,
  pending: false,
}));
const draggableInputs = vi.hoisted(() => [] as Array<{ data: unknown; disabled?: boolean }>);
const backend = vi.hoisted(() => ({ error: null as unknown }));
vi.mock("@dnd-kit/core", () => ({
  useDraggable: (input: { data: unknown; disabled?: boolean }) => {
    draggableInputs.push(input);
    return { attributes: {}, listeners: {}, setNodeRef: vi.fn() };
  },
}));
vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "common.error": "Error",
        "common.loading": "Loading...",
        "common.incidentId": "Incident ID",
        "sidebar.noActiveGraph": "No active graph open",
        "sidebar.noActiveGraphDescription": "Open a graph to view commands",
        "common.undo": "Undo",
        "common.redo": "Redo",
      })[key] ?? key,
  }),
}));
vi.mock("@/features/application/editor", () => ({
  useEditorHistoryAvailability: () => historyAvailability,
}));
vi.mock("@/features/application/sidebar/useActivityPanelDocument", () => ({
  useActivityPanelDocument: (panelId: string) => ({
    document: backend.error ? null : panelId === "nodes" ? nodesDocument : commandsDocument,
    error: backend.error
      ? formatInlineUserError(
          backend.error,
          ((key: string) =>
            ({ "common.error": "Error", "common.incidentId": "Incident ID" })[key] ?? key) as never,
        )
      : null,
    refresh: vi.fn(),
    ...useActivityPanelExpansion(panelId === "nodes" ? "nodes" : "commands"),
  }),
}));
import { SidebarCommandsTab } from "@/modules/commands/internal/ui/activity/SidebarCommandsTab";
import { SidebarNodesTab } from "@/modules/node-catalog/internal/ui/activity/SidebarNodesTab";
const nodeCreation = { kind: "static" as const, nodeTypeId: "yssbi.numeric.add" };
const nodesDocument = activityPanelFixture("nodes", [
  categoryFixture("math", "Math"),
  {
    kind: "item",
    id: "node:add",
    depth: 1,
    item: { kind: "node", key: "static:yssbi.numeric.add", title: "Add", creation: nodeCreation },
  },
]);
const commandsDocument = activityPanelFixture("commands", [
  {
    kind: "item",
    id: "undo",
    depth: 0,
    item: { kind: "command", id: "undo", label: { key: "common.undo" } },
  },
  {
    kind: "item",
    id: "redo",
    depth: 0,
    item: { kind: "command", id: "redo", label: { key: "common.redo" } },
  },
]);
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
describe("Sidebar tab-level empty states", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    draggableInputs.length = 0;
    backend.error = null;
    useSidebarStore.setState({ expandedCategories: {} });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("renders all Catalog items as draggable templates", () => {
    act(() =>
      root.render(
        <TooltipProvider>
          <SidebarNodesTab />
        </TooltipProvider>,
      ),
    );

    expect(host.querySelector("input")).toBeNull();
    const math = host.querySelector<HTMLButtonElement>('[data-sidebar-tree-category-id="math"]')!;
    act(() => math.click());

    expect(host.textContent).toContain("Add");
    expect(host.textContent).not.toContain("yssbi.numeric.add");
    expect(host.querySelector('[title="yssbi.numeric.add"]')).not.toBeNull();

    expect(draggableInputs).toContainEqual({
      id: "sidebar-item-node-static:yssbi.numeric.add",
      disabled: false,
      data: {
        type: "node-template",
        template: {
          title: "Add",
          descriptor: { kind: "static", nodeTypeId: "yssbi.numeric.add" },
        },
      },
    });
    const dragData = draggableInputs[0]?.data as { template?: { descriptor?: unknown } };
    expect(dragData.template?.descriptor).toBe(nodeCreation);
  });

  it("renders localized generic Catalog text, code, and incident ID", () => {
    backend.error = new IpcError({
      kind: "backend",
      command: "get_activity_panel_document",
      code: "catalog_backend_failed",
      incidentId: "incident-sidebar-catalog-42",
      details: null,
      cause: null,
    });

    act(() =>
      root.render(
        <TooltipProvider>
          <SidebarNodesTab />
        </TooltipProvider>,
      ),
    );

    expect(host.textContent).toContain("Error");
    expect(host.textContent).toContain("[catalog_backend_failed]");
    expect(host.textContent).toContain("Incident ID: incident-sidebar-catalog-42");
  });

  it("uses the shared empty state when Commands has no active graph", () => {
    historyAvailability.activeResourceRef = null;
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain("No active graph open");
    expect(host.textContent).toContain("Open a graph to view commands");
    expect(host.querySelector('[data-slot="scroll-area-viewport"]')).toBeNull();
  });

  it("keeps command controls when an active graph exists", () => {
    historyAvailability.activeResourceRef = "events/Main.yssbi-event";
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain("Undo");
    expect(host.textContent).toContain("Redo");
  });
});
