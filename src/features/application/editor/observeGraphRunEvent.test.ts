import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  revokeAllPinPreviewLeases,
  useExecutionStore,
  type PinPreviewLease,
} from "@/features/core/execution";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import { pinPreviewCacheKey } from "@/features/core/execution/pinResultIndex";
import { cancelActiveGraphRun } from "./cancelActiveGraphRun";
import {
  observePinPreviewEvent,
  installGraphRunEvent,
  type PinPreviewObservation,
} from "./observeGraphRunEvent";

function event(kind: RunEvent["kind"]): RunEvent {
  return {
    run: {
      executionSessionId: "project-session-1",
      graphPath: "events/Main.yssbi-event",
      runId: "9007199254740993",
    },
    kind,
  };
}

const declaredOutput: PortAddressDto = {
  kind: "declared",
  nodeId: "node-1",
  portKey: "result",
};

const instanceOutput: PortAddressDto = {
  kind: "instance",
  nodeId: "node-1",
  templateKey: "result",
  instanceId: "instance-7",
};

function beginPreview(
  graphPath: string,
  port: PortAddressDto,
  generation: number,
): PinPreviewLease {
  return useExecutionStore.getState().beginPinPreview(graphPath, port, generation);
}

function previewObservation(
  lease: PinPreviewLease,
  port: PortAddressDto = declaredOutput,
): PinPreviewObservation {
  return {
    executionSessionId: null,
    output: { graphPath: "events/Main.yssbi-event", port },
    generation: lease.generation,
    runId: null,
    terminal: "pending",
    stale: false,
    lease,
  };
}

describe("observeGraphRunEvent", () => {
  it("installs unknown runs from recovery and ignores duplicate starts and older terminal snapshots", () => {
    useExecutionStore.setState({ graphs: {} });
    const started = event({ type: "runStarted", outputs: [] });
    started.run = { ...started.run, executionSessionId: "recovered-session", runId: "51" };
    const path = started.run.graphPath;
    expect(installGraphRunEvent(started)).toBe(true);
    useExecutionStore.getState().markExecutionUnknown(path);
    installGraphRunEvent(started);
    expect(useExecutionStore.getState().getGraph(path).status).toBe("running");
    installGraphRunEvent({ ...started, kind: { type: "runCompleted" } });
    installGraphRunEvent(started);
    expect(useExecutionStore.getState().getGraph(path).status).toBe("completed");
    const next = { ...started, run: { ...started.run, runId: "52" } };
    installGraphRunEvent(next);
    installGraphRunEvent({ ...started, kind: { type: "runCompleted" } });
    expect(useExecutionStore.getState().getGraph(path)).toMatchObject({
      status: "running",
      runId: "52",
    });
  });
  beforeEach(() => {
    startProjectLifecycle("observation-test");
    revokeAllPinPreviewLeases();
    useExecutionStore.setState({
      graphs: {},
    });
  });

  it("projects the runStarted opaque ID into the active graph state", () => {
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().startExecution(graphPath);

    installGraphRunEvent(event({ type: "runStarted", outputs: [] }));

    expect(useExecutionStore.getState().getGraph(graphPath).runId).toBe("9007199254740993");
  });

  it.each([
    ["declared", declaredOutput],
    ["dynamic instance", instanceOutput],
  ] as const)("completes a %s preview only from its exact pinPreviewResultReady", (_name, port) => {
    const graphPath = "events/Main.yssbi-event";
    const lease = beginPreview(graphPath, port, 1);
    const generation = lease.generation;
    const preview = previewObservation(lease, port);

    observePinPreviewEvent(graphPath, event({ type: "runStarted", outputs: [] }), preview);
    observePinPreviewEvent(
      graphPath,
      event({
        type: "pinPreviewResultReady",
        output: { graphPath, port },
        generation,
        resultId: "result-current",
      }),
      preview,
    );

    expect(
      useExecutionStore
        .getState()
        .getGraph(graphPath)
        .pinPreviews.get(pinPreviewCacheKey(graphPath, port)),
    ).toMatchObject({ status: "ready", resultId: "result-current" });
  });

  it("ignores mismatched output identity, backend session, run, and stale generation", () => {
    const graphPath = "events/Main.yssbi-event";
    const staleLease = beginPreview(graphPath, declaredOutput, 1);
    const currentLease = beginPreview(graphPath, declaredOutput, 2);
    const staleGeneration = staleLease.generation;
    const currentGeneration = currentLease.generation;
    const stale = previewObservation(staleLease);
    const current = previewObservation(currentLease);

    observePinPreviewEvent(graphPath, event({ type: "runStarted", outputs: [] }), stale);
    observePinPreviewEvent(graphPath, event({ type: "runStarted", outputs: [] }), current);
    observePinPreviewEvent(
      graphPath,
      event({
        type: "pinPreviewResultReady",
        output: { graphPath, port: instanceOutput },
        generation: currentGeneration,
        resultId: "result-wrong-address",
      }),
      current,
    );
    const wrongSession = event({
      type: "pinPreviewResultReady",
      output: { graphPath, port: declaredOutput },
      generation: currentGeneration,
      resultId: "result-stale-session",
    });
    wrongSession.run.executionSessionId = "stale-backend-session";
    observePinPreviewEvent(graphPath, wrongSession, current);
    const wrongRun = event({
      type: "pinPreviewResultReady",
      output: { graphPath, port: declaredOutput },
      generation: currentGeneration,
      resultId: "result-wrong-run",
    });
    wrongRun.run.runId = "different-run";
    observePinPreviewEvent(graphPath, wrongRun, current);
    observePinPreviewEvent(
      graphPath,
      event({
        type: "pinPreviewResultReady",
        output: { graphPath, port: declaredOutput },
        generation: staleGeneration,
        resultId: "result-stale-generation",
      }),
      stale,
    );

    expect(
      useExecutionStore
        .getState()
        .getGraph(graphPath)
        .pinPreviews.get(pinPreviewCacheKey(graphPath, declaredOutput)),
    ).toMatchObject({
      generation: currentGeneration,
      status: "pending",
      resultId: null,
    });
  });

  it("ignores exact old-run events locally after the same-pin lease is revoked", () => {
    const graphPath = "events/Main.yssbi-event";
    const staleLease = beginPreview(graphPath, declaredOutput, 1);
    beginPreview(graphPath, declaredOutput, 2);
    const stale = previewObservation(staleLease);
    const store = useExecutionStore.getState();
    const completePinPreview = vi.spyOn(store, "completePinPreview");
    const failPinPreview = vi.spyOn(store, "failPinPreview");
    const getExecutionState = vi.spyOn(useExecutionStore, "getState");

    observePinPreviewEvent(graphPath, event({ type: "runStarted", outputs: [] }), stale);
    observePinPreviewEvent(
      graphPath,
      event({
        type: "pinPreviewResultReady",
        output: { graphPath, port: declaredOutput },
        generation: 1,
        resultId: "result-old",
      }),
      stale,
    );
    observePinPreviewEvent(graphPath, event({ type: "runCompleted" }), stale);

    expect(stale).toMatchObject({ runId: null, terminal: "pending", stale: false });
    expect(getExecutionState).not.toHaveBeenCalled();
    expect(completePinPreview).not.toHaveBeenCalled();
    expect(failPinPreview).not.toHaveBeenCalled();
  });

  it.each([
    [{ type: "runCompleted" } as const, "completed"],
    [
      { type: "runErrored", code: "kernelFailed", phase: "execution", source: null } as const,
      "error",
    ],
    [{ type: "runCancelled" } as const, "cancelled"],
  ])("keeps preview $type isolated from an active ordinary run", (terminal, expectedTerminal) => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();
    store.startExecution(graphPath);
    store.setActiveRunId(graphPath, "ordinary-run");
    const lease = beginPreview(graphPath, declaredOutput, 1);
    const generation = lease.generation;
    const preview = previewObservation(lease);

    const previewStarted = event({ type: "runStarted", outputs: [] });
    previewStarted.run.runId = "preview-run";
    observePinPreviewEvent(graphPath, previewStarted, preview);
    const previewResultReady = event({
      type: "pinPreviewResultReady",
      output: { graphPath, port: declaredOutput },
      generation,
      resultId: "preview-result",
    });
    previewResultReady.run.runId = "preview-run";
    observePinPreviewEvent(graphPath, previewResultReady, preview);
    const previewTerminal = event(terminal);
    previewTerminal.run.runId = "preview-run";
    observePinPreviewEvent(graphPath, previewTerminal, preview);

    expect(useExecutionStore.getState().getGraph(graphPath)).toMatchObject({
      status: "running",
      runId: "ordinary-run",
    });
    expect(preview.terminal).toBe(expectedTerminal);
    expect(
      useExecutionStore
        .getState()
        .getGraph(graphPath)
        .pinPreviews.get(pinPreviewCacheKey(graphPath, declaredOutput)),
    ).toMatchObject({ status: "ready", resultId: "preview-result" });
  });

  it("keeps the ordinary run as the cancellation target after preview runStarted", async () => {
    const graphPath = "events/Main.yssbi-event";
    const store = useExecutionStore.getState();
    store.startExecution(graphPath);
    store.setActiveRunId(graphPath, "ordinary-run");
    const lease = beginPreview(graphPath, declaredOutput, 1);
    const preview = previewObservation(lease);
    const previewStarted = event({ type: "runStarted", outputs: [] });
    previewStarted.run.runId = "preview-run";

    observePinPreviewEvent(graphPath, previewStarted, preview);
    const cancelGraphRun = vi.fn().mockResolvedValue(true);
    await cancelActiveGraphRun(graphPath, { cancelGraphRun });

    expect(cancelGraphRun).toHaveBeenCalledWith("ordinary-run");
    expect(useExecutionStore.getState().getGraph(graphPath)).toMatchObject({
      status: "running",
      runId: "ordinary-run",
    });
  });

  it("classifies canonical terminal events", () => {
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().startExecution(graphPath);
    installGraphRunEvent(event({ type: "runStarted", outputs: [] }));

    installGraphRunEvent(
      event({ type: "runErrored", code: "kernelFailed", phase: "execution", source: null }),
    );
    expect(useExecutionStore.getState().getGraph(graphPath).runFailure).toMatchObject({
      code: "kernelFailed",
      phase: "execution",
      runId: "9007199254740993",
    });

    expect(useExecutionStore.getState().getGraph(graphPath).status).toBe("error");
    const nextRun = {
      ...event({ type: "runStarted", outputs: [] }),
      run: { ...event({ type: "runCancelled" }).run, runId: "9007199254740994" },
    };
    installGraphRunEvent(nextRun);
    installGraphRunEvent({ ...nextRun, kind: { type: "runCancelled" } });
    expect(useExecutionStore.getState().getGraph(graphPath).status).toBe("idle");
  });
});
