// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { MemoryRouter, useLocation, useNavigate, type NavigateFunction } from "react-router";
import type { ReactFlowProps } from "@xyflow/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ArchitectureModal } from "./ArchitectureModal";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@xyflow/react", async (original) => ({
  ...(await original<typeof import("@xyflow/react")>()),
  Handle: () => null,
  // Exercise the real cards and Router/Dialog interactions without a browser layout engine.
  ReactFlow: ({ nodes = [], nodeTypes = {} }: ReactFlowProps) => (
    <div>
      {nodes.map((node) => {
        const NodeComponent = nodeTypes[node.type!];
        return (
          <div key={node.id} data-architecture-node={node.id}>
            <NodeComponent
              id={node.id}
              type={node.type!}
              data={node.data}
              positionAbsoluteX={node.position.x}
              positionAbsoluteY={node.position.y}
              selected={false}
              dragging={false}
              draggable={false}
              selectable={false}
              deletable={false}
              isConnectable={false}
              zIndex={0}
            />
          </div>
        );
      })}
    </div>
  ),
}));

describe("Architecture route navigation", () => {
  let host: HTMLDivElement;
  let root: Root;
  let currentLocation: string;
  let navigate: NavigateFunction;

  function RouteProbe() {
    const location = useLocation();
    currentLocation = `${location.pathname}${location.search}${location.hash}`;
    navigate = useNavigate();
    return null;
  }

  async function renderRoute(route: string) {
    await act(async () =>
      root.render(
        <MemoryRouter initialEntries={[route]}>
          <TooltipProvider>
            <ArchitectureModal />
          </TooltipProvider>
          <RouteProbe />
        </MemoryRouter>,
      ),
    );
  }

  async function click(selector: string) {
    const target = document.querySelector<HTMLElement>(selector);
    expect(target).not.toBeNull();
    await act(async () =>
      target!.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true })),
    );
  }

  beforeEach(() => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it.each([
    { view: "frontend", nodeCount: 10, page: "frontendPage" },
    { view: "backend", nodeCount: 9, page: "backendPage" },
    { view: "communication", nodeCount: 8, page: "communicationPage" },
  ])(
    "keeps $view drill-down, breadcrumbs and history in sync",
    async ({ view, nodeCount, page }) => {
      await renderRoute("/editor?keep=one&keep=two&architecture=overview#section");
      expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(3);

      await click(`[data-section="${view}"]`);
      expect(currentLocation).toBe(`/editor?keep=one&keep=two&architecture=${view}#section`);
      expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(nodeCount);
      expect(document.querySelector('nav [aria-current="page"]')?.textContent).toBe(
        `architectureModal.${page}`,
      );

      await click('nav[aria-label="architectureModal.breadcrumb"] a');
      expect(currentLocation).toContain("architecture=overview");
      expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(3);

      await act(async () => navigate(-1));
      expect(currentLocation).toContain(`architecture=${view}`);
      expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(nodeCount);

      await act(async () => navigate(1));
      expect(currentLocation).toContain("architecture=overview");
      expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(3);

      await click(`a[href*="architecture=${view}"]`);
      expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(nodeCount);
    },
  );

  it("opens a backend deep link and clears only its route parameter when closed", async () => {
    await renderRoute("/editor?keep=one&keep=two&architecture=backend#section");
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
    expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(9);

    await click('[data-architecture-node="project"] button');
    expect(document.querySelector('[data-slot="popover-content"]')?.textContent).toContain(
      "yss-project-watcher-notify",
    );

    await click('button[aria-label="common.close"]');
    expect(currentLocation).toBe("/editor?keep=one&keep=two#section");
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });

  it("opens a frontend deep link and shows implementation details before closing", async () => {
    await renderRoute("/editor?keep=one&keep=two&architecture=frontend#section");
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
    expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(10);

    await click('[data-architecture-node="workbench"] button');
    expect(document.querySelector('[data-slot="popover-content"]')?.textContent).toContain(
      "src/modules/workbench/",
    );

    await click('button[aria-label="common.close"]');
    expect(currentLocation).toBe("/editor?keep=one&keep=two#section");
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });

  it("opens an IPC deep link with its error contract and clears only its route parameter", async () => {
    await renderRoute("/editor?keep=one&keep=two&architecture=communication#section");
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
    expect(document.querySelectorAll("[data-architecture-node]")).toHaveLength(8);

    await click('[data-architecture-node="contract"] button');
    const contract = document.querySelector('[data-slot="popover-content"]');
    expect(JSON.parse(contract!.querySelector("pre")!.textContent!)).toEqual({
      code: "project_not_found",
      details: null,
      incidentId: null,
    });

    await click('button[aria-label="common.close"]');
    expect(currentLocation).toBe("/editor?keep=one&keep=two#section");
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });
});
