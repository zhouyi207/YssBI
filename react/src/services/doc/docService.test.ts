import { expect, it, vi } from "vitest";
import { invokeCommand } from "@/services/ipc";
import { DocService, parseDocSnapshot } from "./docService";
import { parseMindSnapshot } from "@/services/mind/mindService";

vi.mock("@/services/ipc", () => ({ invokeCommand: vi.fn() }));

it("routes Doc reads independently and rejects cross-kind snapshots and content", async () => {
  const snapshot = {
    projectInstanceId: "project-a",
    path: "docs/Report.md",
    kind: "doc",
    content: "# Report",
    version: { sessionId: "session", revision: 1 },
    dirty: false,
  };
  vi.mocked(invokeCommand).mockResolvedValueOnce(snapshot);
  expect(await DocService.read("project-a", snapshot.path)).toEqual(snapshot);
  expect(invokeCommand).toHaveBeenCalledWith("read_project_doc", {
    projectInstanceId: "project-a",
    path: snapshot.path,
  });
  expect(() => parseMindSnapshot(snapshot)).toThrow();
  expect(() => parseDocSnapshot({ ...snapshot, kind: "mind" })).toThrow();
  expect(() => parseDocSnapshot({ ...snapshot, path: "minds/Plan.yssbi-mind" })).toThrow();
  expect(() => parseDocSnapshot({ ...snapshot, content: { rootId: "root", nodes: [] } })).toThrow();
});
