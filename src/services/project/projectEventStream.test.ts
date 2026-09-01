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
    expect(received[0]).toMatchObject({ kind: "event", event: { type: "GraphDelta" } });

    unsubscribe();
    unsubscribe();
    callback({ payload: projectEvents.events[1] });
    expect(received).toHaveLength(1);

    await stream.close();
    await stream.close();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("converts parser rejection to an opaque failure item without backend prose", async () => {
    listenMock.mockResolvedValue(vi.fn());
    const stream = createProjectEventStream();
    const received: ProjectEventStreamItem[] = [];
    stream.subscribe((item) => received.push(item));
    expect(await stream.start()).toEqual({ ok: true, value: undefined });

    const callback = listenMock.mock.calls[0][1] as (event: { payload: unknown }) => void;
    callback({
      payload: {
        type: "Project",
        payload: {
          type: "GraphDelta",
          payload: { projectInstanceId: "project-1", message: "backend prose" },
        },
      },
    });

    expect(received).toEqual([
      {
        kind: "failure",
        issue: { code: "project_event_invalid_payload", incidentId: null },
      },
    ]);
    expect(JSON.stringify(received)).not.toContain("backend prose");
    await stream.close();
  });

  it("returns a typed opaque subscription failure and closes without leaking the rejection", async () => {
    listenMock.mockRejectedValue(new Error("private subscription prose"));
    const stream = createProjectEventStream();

    expect(await stream.start()).toEqual({
      ok: false,
      issue: { code: "project_event_subscription_failed", incidentId: null },
    });
    expect(JSON.stringify(await stream.start())).not.toContain("private subscription prose");
    await stream.close();
  });
});
