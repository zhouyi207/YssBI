import { describe, expect, it } from 'vitest';
import type { RunEvent, RunOutputChannelEvent } from '@/shared/types/dto/runEvent';
import type { RecordedEvent } from '@/shared/types/ui/execution';
import {
  ExecutionProjectionCoordinator,
  type ExecutionProjectionPublication,
} from './executionProjectionCoordinator';

const run = {
  projectSessionId: 'project-a',
  graphPath: 'events/example.yssbi-event',
  runId: 'run-1',
};

const start: RunEvent = { run, kind: { type: 'runStarted' } };
const inspection: RunEvent = {
  run,
  kind: {
    type: 'resultInspectionRequested',
    resultId: '17',
    source: {
      graphPath: 'functions/Inspect.yssbi-function',
      nodeId: null,
      portAddress: null,
    },
  },
};
const complete: RunEvent = { run, kind: { type: 'runCompleted' } };
const output: RunOutputChannelEvent = {
  runId: 'run-1',
  sequence: 1,
  stream: 'stdout',
  text: 'done',
  sourceGraphPath: run.graphPath,
  sourceNodeId: 'node-1',
};

describe('ExecutionProjectionCoordinator', () => {
  it('keeps ordered run/output publication separate from recording/playback UI and never opens a window', () => {
    const calls: string[] = [];
    const publication: ExecutionProjectionPublication = {
      startRun: (graphPath, runId) => calls.push(`start:${graphPath}:${runId}`),
      applyRunEvent: (event) => calls.push(`event:${event.kind.type}`),
      applyRunOutput: (event) => calls.push(`output:${event.sequence}`),
      applyPinHistory: () => calls.push('pin-history'),
      finishRun: (graphPath) => calls.push(`finish:${graphPath}`),
      clearForProject: (projectId) => calls.push(`clear:${projectId ?? 'none'}`),
    };
    const ui = {
      setRecording: (graphPath: string, recording: readonly RecordedEvent[]) => {
        calls.push(`recording:${graphPath}:${recording.length}`);
      },
      setPlaying: (playing: boolean, graphPath?: string) => {
        calls.push(`playing:${playing}:${graphPath ?? 'none'}`);
      },
      resetVisuals: (graphPath: string) => calls.push(`visuals:${graphPath}`),
    };
    const coordinator = new ExecutionProjectionCoordinator({ publication, ui });

    coordinator.publish(start);
    coordinator.publish(output);
    coordinator.publish(inspection);
    coordinator.publish(complete);
    coordinator.setRecording(run.graphPath, []);
    coordinator.setPlaying(true, run.graphPath);
    coordinator.resetVisuals(run.graphPath);

    expect(calls).toEqual([
      `start:${run.graphPath}:run-1`,
      'output:1',
      'event:resultInspectionRequested',
      'event:runCompleted',
      `finish:${run.graphPath}`,
      `recording:${run.graphPath}:0`,
      `playing:true:${run.graphPath}`,
      `visuals:${run.graphPath}`,
    ]);
  });
});
