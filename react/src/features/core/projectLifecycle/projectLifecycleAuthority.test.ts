import { beforeEach, describe, expect, it, vi } from "vitest";

const projectA = "00000000-0000-0000-0000-000000000601";
const projectB = "00000000-0000-0000-0000-000000000602";

type ProjectLifecycleAuthority =
  typeof import("@/features/core/projectLifecycle/projectLifecycleAuthority");

let authority: ProjectLifecycleAuthority;

describe("projectLifecycleAuthority", () => {
  beforeEach(async () => {
    vi.resetModules();
    authority = await import("@/features/core/projectLifecycle/projectLifecycleAuthority");
  });

  it("rejects capture when no project lifecycle is active", () => {
    expect(() => authority.captureProjectIdentity()).toThrow(
      expect.objectContaining({
        code: "stale_project_lifecycle",
        message: "project lifecycle changed before publication settlement",
      }),
    );
  });

  it("captures the first active project lifecycle", () => {
    authority.startProjectLifecycle(projectA);

    const snapshot = authority.captureProjectIdentity();

    expect(snapshot.projectInstanceId).toBe(projectA);
    expect(snapshot.epoch).toBeGreaterThan(0);
    expect(authority.isCurrentProjectIdentity(snapshot)).toBe(true);
    expect(() => authority.assertCurrentProjectIdentity(snapshot)).not.toThrow();
  });

  it("invalidates a captured lifecycle when another project replaces it", () => {
    authority.startProjectLifecycle(projectA);
    const stale = authority.captureProjectIdentity();

    authority.startProjectLifecycle(projectB);
    const current = authority.captureProjectIdentity();

    expect(current.projectInstanceId).toBe(projectB);
    expect(current.epoch).toBeGreaterThan(stale.epoch);
    expect(authority.isCurrentProjectIdentity(stale)).toBe(false);
    expect(authority.isCurrentProjectIdentity(current)).toBe(true);
  });

  it("invalidates a captured lifecycle when the active project is cleared", () => {
    authority.startProjectLifecycle(projectA);
    const stale = authority.captureProjectIdentity();

    authority.clearProjectLifecycle();

    expect(authority.isCurrentProjectIdentity(stale)).toBe(false);
    expect(() => authority.captureProjectIdentity()).toThrow(
      expect.objectContaining({ code: "stale_project_lifecycle" }),
    );
  });

  it("returns an immutable lifecycle snapshot", () => {
    authority.startProjectLifecycle(projectA);

    const snapshot = authority.captureProjectIdentity();

    expect(Object.isFrozen(snapshot)).toBe(true);
    expect(() => {
      (snapshot as { projectInstanceId: string }).projectInstanceId = projectB;
    }).toThrow();
    expect(authority.captureProjectIdentity()).toEqual(snapshot);
  });

  it("throws the compatible stale lifecycle error for an invalidated snapshot", () => {
    authority.startProjectLifecycle(projectA);
    const stale = authority.captureProjectIdentity();
    authority.startProjectLifecycle(projectB);

    expect(() => authority.assertCurrentProjectIdentity(stale)).toThrow(
      expect.objectContaining({
        code: "stale_project_lifecycle",
        message: "project lifecycle changed before publication settlement",
      }),
    );
  });

  it("treats repeated activation of the same project as a new lifecycle", () => {
    authority.startProjectLifecycle(projectA);
    const stale = authority.captureProjectIdentity();

    authority.startProjectLifecycle(projectA);
    const current = authority.captureProjectIdentity();

    expect(current.projectInstanceId).toBe(projectA);
    expect(current.epoch).toBeGreaterThan(stale.epoch);
    expect(authority.isCurrentProjectIdentity(stale)).toBe(false);
    expect(authority.isCurrentProjectIdentity(current)).toBe(true);
  });

  it("captures and compares lifecycle state while no project is active", () => {
    const inactive = authority.captureProjectLifecycleState();

    expect(inactive.projectInstanceId).toBeNull();
    expect(Object.isFrozen(inactive)).toBe(true);
    expect(authority.isProjectLifecycleStateCurrent(inactive)).toBe(true);

    authority.startProjectLifecycle(projectA);

    expect(authority.isProjectLifecycleStateCurrent(inactive)).toBe(false);
  });

  it("retains the latest activation revision after clearing the active lifecycle", () => {
    expect(authority.acceptProjectLifecycleActivation(projectA, 2000)).toBe("activated");

    authority.clearProjectLifecycle();

    expect(authority.acceptProjectLifecycleActivation(projectB, 1999)).toBe("stale");
    expect(() => authority.captureProjectIdentity()).toThrow(
      expect.objectContaining({ code: "stale_project_lifecycle" }),
    );
  });

  it("deduplicates activation receipts and rejects older replacements", () => {
    expect(authority.acceptProjectLifecycleActivation(projectA, 1001)).toBe("activated");
    const first = authority.captureProjectIdentity();

    expect(authority.acceptProjectLifecycleActivation(projectA, 1001)).toBe("duplicate");
    expect(authority.captureProjectIdentity()).toEqual(first);
    expect(authority.acceptProjectLifecycleActivation(projectB, 1000)).toBe("stale");
    expect(authority.captureProjectIdentity()).toEqual(first);

    expect(authority.acceptProjectLifecycleActivation(projectB, 1002)).toBe("activated");
    expect(authority.captureProjectIdentity().projectInstanceId).toBe(projectB);
    expect(authority.isCurrentProjectIdentity(first)).toBe(false);
  });
});
