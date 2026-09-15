import { resultReferenceFixture, resultLeaseIdFixture } from "@/tests/helpers/resultFixture";
import { describe, expect, it } from "vitest";

import {
  componentForWorkbenchMetadata,
  isWorkbenchPanelMetadata,
  type ResultPanelMetadata,
  type ViewPanelMetadata,
  type WorkbenchComponentId,
} from "./workbenchPanelModel";

describe("workbench panel metadata", () => {
  it("keeps canonical editor identity and component selection in one place", () => {
    const metadata = {
      role: "editor",
      resourceRef: "events/Main.yssbi-event",
      resourceKind: "event",
      sticky: true,
    } as const;

    expect(isWorkbenchPanelMetadata(metadata)).toBe(true);
    expect(componentForWorkbenchMetadata(metadata)).toBe("EditorResource");

    expect(
      componentForWorkbenchMetadata({
        role: "editor",
        resourceRef: "charts/Model.yssbi-chart",
        resourceKind: "chart",
      }),
    ).toBe("EditorResource");
  });

  it("accepts every view and canonical result presentation/source variant", () => {
    const viewCases: readonly {
      metadata: ViewPanelMetadata;
      component: WorkbenchComponentId;
    }[] = [
      { metadata: { role: "view", viewId: "project" }, component: "Project" },
      { metadata: { role: "view", viewId: "nodes" }, component: "Nodes" },
      { metadata: { role: "view", viewId: "commands" }, component: "Commands" },
      { metadata: { role: "view", viewId: "details" }, component: "Details" },
      { metadata: { role: "view", viewId: "inspect" }, component: "Inspect" },
      { metadata: { role: "view", viewId: "logs" }, component: "Logs" },
      { metadata: { role: "view", viewId: "output" }, component: "Output" },
      { metadata: { role: "view", viewId: "problems" }, component: "Problems" },
    ];
    for (const { metadata: view, component } of viewCases) {
      expect(isWorkbenchPanelMetadata(view)).toBe(true);
      expect(componentForWorkbenchMetadata(view)).toBe(component);
    }

    const results: readonly ResultPanelMetadata[] = [
      {
        role: "result",
        leaseId: resultLeaseIdFixture(1),
        reference: resultReferenceFixture("41"),
        title: "Inspector result",
        presentation: { kind: "inspector" },
      },
      {
        role: "result",
        leaseId: resultLeaseIdFixture(2),
        reference: resultReferenceFixture("42"),
        title: "Plot result",
        presentation: { kind: "plot", chart: "scatter" },
      },
      {
        role: "result",
        leaseId: resultLeaseIdFixture(3),
        reference: resultReferenceFixture("43"),
        title: "Report result",
        presentation: { kind: "report", report: "olsSummary" },
      },
    ];
    for (const result of results) {
      expect(isWorkbenchPanelMetadata(result)).toBe(true);
      expect(componentForWorkbenchMetadata(result)).toBe("Result");
    }
  });

  it("rejects empty, unknown, and obsolete metadata structures", () => {
    const invalidMetadata: unknown[] = [
      {
        role: "editor",
        resourceRef: "",
        resourceKind: "event",
      },
      {
        role: "editor",
        resourceRef: "settings",
        resourceKind: "project",
      },
      {
        role: "editor",
        resourceRef: "settings",
        resourceKind: "setting",
      },
      {
        role: "editor",
        resourceRef: "events/Main.yssbi-event",
        resourceKind: "event",
        legacyId: "old",
      },
      {
        role: "editor",
        resourceRef: "events/Main.yssbi-event",
        resourceKind: "event",
        pinned: true,
      },
      { role: "view", viewId: "result" },
      { role: "view", viewId: "settings" },
      { role: "unknown", viewId: "obsolete-view" },
      {
        role: "result",
        leaseId: "",
        reference: resultReferenceFixture("42"),
        title: "Result",
        presentation: { kind: "inspector" },
      },
      {
        role: "result",
        leaseId: resultLeaseIdFixture(5),
        reference: resultReferenceFixture(""),
        title: "Result",
        presentation: { kind: "inspector" },
      },
    ];

    for (const metadata of invalidMetadata) {
      expect(isWorkbenchPanelMetadata(metadata)).toBe(false);
    }
  });
});
