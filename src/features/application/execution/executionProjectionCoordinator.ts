import type { ExecutionUi } from "@/features/core/execution/ui";
import type { RunEvent, RunOutputChannelEvent } from "@/shared/types/domain/runEvent";
import type { PinHistoryProjection, RecordedEvent } from "@/shared/types/ui/execution";

export interface ExecutionProjectionPublication {
  readonly startRun: (graphPath: string, runId: string) => void;
  readonly applyRunEvent: (event: RunEvent) => void;
  readonly applyRunOutput: (event: RunOutputChannelEvent) => void;
  readonly applyPinHistory: (projection: PinHistoryProjection) => void;
  readonly finishRun: (graphPath: string) => void;
  readonly clearForProject: (projectInstanceId: string | null) => void;
}

export type ExecutionChannelEvent = RunEvent | RunOutputChannelEvent;

export interface ExecutionProjectionCoordinatorDependencies {
  readonly publication: ExecutionProjectionPublication;
  readonly ui?: ExecutionUi;
}

function isRunEvent(event: ExecutionChannelEvent): event is RunEvent {
  return "run" in event && "kind" in event;
}

function isTerminalRunEvent(event: RunEvent): boolean {
  return (
    event.kind.type === "runCompleted" ||
    event.kind.type === "runErrored" ||
    event.kind.type === "runCancelled"
  );
}

/**
 * Application-owned ingress for the ordered run channel.
 *
 * The coordinator forwards already parsed events in call order. It owns no
 * window side effect: an inspection request remains a neutral RunEvent for
 * the Presentation handler that owns that workflow.
 */
export class ExecutionProjectionCoordinator {
  private readonly publication: ExecutionProjectionPublication;
  private readonly ui: ExecutionUi | undefined;

  constructor(dependencies: ExecutionProjectionCoordinatorDependencies) {
    this.publication = dependencies.publication;
    this.ui = dependencies.ui;
  }

  publish(event: ExecutionChannelEvent): void {
    if (isRunEvent(event)) {
      this.publishRunEvent(event);
      return;
    }
    this.publishRunOutput(event);
  }

  publishRunEvent(event: RunEvent): void {
    if (event.kind.type === "runStarted") {
      this.publication.startRun(event.run.graphPath, event.run.runId);
      return;
    }

    this.publication.applyRunEvent(event);
    if (isTerminalRunEvent(event)) {
      this.publication.finishRun(event.run.graphPath);
    }
  }

  publishRunOutput(event: RunOutputChannelEvent): void {
    this.publication.applyRunOutput(event);
  }

  publishPinHistory(projection: PinHistoryProjection): void {
    this.publication.applyPinHistory(projection);
  }

  clearForProject(projectInstanceId: string | null): void {
    this.publication.clearForProject(projectInstanceId);
  }

  setRecording(graphPath: string, recording: readonly RecordedEvent[]): void {
    this.ui?.setRecording(graphPath, recording);
  }

  setPlaying(playing: boolean, graphPath?: string): void {
    this.ui?.setPlaying(playing, graphPath);
  }

  resetVisuals(graphPath: string): void {
    this.ui?.resetVisuals(graphPath);
  }
}
