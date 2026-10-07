import { beforeEach, describe, expect, it, vi } from "vitest";
import { uiStore } from "@/features/core/ui/UIStore";
import { deleteResource } from "@/features/application/resource/resourceActions";
import { deleteFileWithConfirm } from "@/features/application/resource/fileManagement";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";

vi.mock("@/features/application/resource/resourceActions", () => ({
  deleteResource: vi.fn(async () => undefined),
}));

describe("deleteFileWithConfirm backend cascade", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    startProjectLifecycle("project-a");
  });

  it("uses one confirmation and delegates function removal to the canonical backend cascade", async () => {
    const confirm = vi.spyOn(uiStore, "confirm").mockResolvedValue(true);

    await expect(
      deleteFileWithConfirm({ id: "functions/Target.yssbi-function", kind: "function_graph" }),
    ).resolves.toBe(true);

    expect(confirm).toHaveBeenCalledOnce();
    expect(deleteResource).toHaveBeenCalledWith({
      id: "functions/Target.yssbi-function",
      kind: "function_graph",
    });
  });
});
