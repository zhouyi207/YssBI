import { expect, it, vi } from "vitest";
import { invokeCommand } from "@/services/ipc";
import { GraphService } from "./graphService";

vi.mock("@/services/ipc", () => ({ invokeCommand: vi.fn() }));

it("accepts only boolean graph unload acknowledgements", async () => {
  const path = "events/Analysis.yssbi-event";
  const version = { sessionId: "editing-session", revision: "3" };
  vi.mocked(invokeCommand).mockResolvedValueOnce("false");
  await expect(GraphService.unloadProjectGraph(path, 7, "project-a", version)).rejects.toThrow();
  for (const removed of [false, true]) {
    vi.mocked(invokeCommand).mockResolvedValueOnce(removed);
    await expect(GraphService.unloadProjectGraph(path, 7, "project-a", version)).resolves.toBe(
      removed,
    );
  }
  expect(invokeCommand).toHaveBeenLastCalledWith("unload_project_graph", {
    graphPath: path,
    lifecycleToken: 7,
    projectInstanceId: "project-a",
    discardVersion: version,
  });
});
