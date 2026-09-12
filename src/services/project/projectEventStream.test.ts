import { beforeEach, describe, expect, it, vi } from "vitest";
import projectEvents from "@/tests/fixtures/node-system-contracts/project-events.json";

const { listenMock } = vi.hoisted(() => ({ listenMock: vi.fn() }));

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { createProjectEventStream, type ProjectEventStreamItem } from "./projectEventStream";

describe("project event stream", () => {
  beforeEach(() => {
    listenMock.mockReset();
  });

  it("owns one raw listener and exposes ordered typed items through subscribe", async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    const stream = createProjectEventStream();
    const received: ProjectEventStreamItem[] = [];
    const unsubscribe = stream.subscribe((item) => received.push(item));

    expect(await stream.start()).toEqual({ ok: true, value: undefined });
    expect(await stream.start()).toEqual({ ok: true, value: undefined });
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledWith("project-event", expect.any(Function));

    const callback = listenMock.mock.calls[0][1] as (event: { payload: unknown }) => void;
    callback({ payload: projectEvents.events[0] });
    expect(received).toHaveLength(1);
    expect(received[0]).toMatchObject({
      kind: "event",
      event: { type: "ResourceMutationCommitted" },
    });

    unsubscribe();
    unsubscribe();
    callback({ payload: projectEvents.events[1] });
    expect(received).toHaveLength(1);

    await stream.close();
    await stream.close();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
