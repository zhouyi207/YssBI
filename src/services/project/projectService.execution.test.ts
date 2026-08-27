import { Channel, invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { RunEvent, RunOutputChannelEvent } from '@/shared/types/dto/runEvent';
import type { ExecutionDemandDto } from '@/shared/types/dto/executionDemand';
import { disposeTrackedChannelsForHmr } from '@/services/devHmrIpc';
import { IpcError } from '@/services/ipc';
import {
  PICKER_TASK_CANCELLED,
  ProjectService,
  isExecutionCancelledError,
  isPickerTaskCancelledError,
} from './projectService';

const projectInstanceId = 'project-instance-1';

vi.mock('@tauri-apps/api/core', () => {
  class TestChannel<T> {
    onmessage?: (message: T) => void;
  }

  return {
    Channel: TestChannel,
    invoke: vi.fn(),
  };
});

function backendIpcError(
  code: string,
  details: Record<string, unknown> | null = null,
): IpcError {
  return new IpcError({
    kind: 'backend',
    command: 'test_command',
    code,
    details,
    incidentId: null,
    cause: null,
  });
}

function runEvent(kind: RunEvent['kind']): RunEvent {
  return {
    run: {
      projectSessionId: 'project-session-1',
      graphPath: 'events/Main.yssbi-event',
      runId: '41',
    },
    kind,
  };
}

describe('ProjectService execution contract', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes cancel_graph_run with an opaque decimal run ID', async () => {
    vi.mocked(invoke).mockResolvedValue(true);

    await expect(ProjectService.cancelGraphRun('9007199254740993')).resolves.toBe(true);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith('cancel_graph_run', {
      runId: '9007199254740993',
    });
  });

  it('classifies only the normalized canonical run cancellation error', () => {
    expect(isExecutionCancelledError(backendIpcError('run_cancelled'))).toBe(true);
    expect(isExecutionCancelledError({
      code: 'run_cancelled',
      details: null,
      incidentId: null,
    })).toBe(false);
    expect(isExecutionCancelledError('EXECUTION_CANCELLED')).toBe(false);
  });

  it('classifies picker cancellation by normalized code instead of a message string', () => {
    expect(isPickerTaskCancelledError(backendIpcError(PICKER_TASK_CANCELLED))).toBe(true);
    expect(isPickerTaskCancelledError('PICKER_TASK_CANCELLED')).toBe(false);
    expect(isPickerTaskCancelledError({ code: PICKER_TASK_CANCELLED })).toBe(false);
  });

  it('preserves a typed internal compilation failure from command IPC', async () => {
    const commandError = {
      code: 'internal_compilation_failure',
      details: {
        internalCompilationFailure: {
          stage: 'lowering',
          code: 'compiler.lowering.internal_invariant',
          nodeId: '00000000-0000-0000-0000-00000000002a',
        },
      },
      incidentId: 'incident-internal-compilation',
    };
    vi.mocked(invoke).mockRejectedValue(commandError);

    await expect(ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
    )).rejects.toMatchObject({
      kind: 'backend',
      code: 'internal_compilation_failure',
      details: commandError.details,
      incidentId: commandError.incidentId,
      cause: commandError,
    });
  });

  it('rejects malformed internal compilation failure details at the service boundary', async () => {
    vi.mocked(invoke).mockRejectedValue({
      code: 'internal_compilation_failure',
      details: {
        internalCompilationFailure: {
          stage: 'lowering',
          code: 'compiler.lowering.internal_invariant',
        },
      },
      incidentId: null,
    });

    await expect(ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
    )).rejects.toThrow('Invalid internal compilation failure response');
  });

  it.each([
    {
      name: 'declared output',
      demand: {
        type: 'outputs' as const,
        outputs: [{
          graphPath: 'events/Main.yssbi-event',
          port: {
            kind: 'declared' as const,
            nodeId: '00000000-0000-0000-0000-000000000002',
            portKey: 'result',
          },
        }],
        includeDefaultResults: false,
      },
    },
    {
      name: 'dynamic output instance',
      demand: {
        type: 'outputs' as const,
        outputs: [{
          graphPath: 'events/Main.yssbi-event',
          port: {
            kind: 'instance' as const,
            nodeId: '00000000-0000-0000-0000-000000000002',
            templateKey: 'value',
            instanceId: '00000000-0000-0000-0000-000000000003',
          },
        }],
        includeDefaultResults: false,
      },
    },
  ])('forwards the exact $name demand', async ({ demand }) => {
    const commandError = { code: 'test_stop', details: null, incidentId: null };
    vi.mocked(invoke).mockRejectedValue(commandError);

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      demand,
    );

    await expect(execution).rejects.toMatchObject({
      kind: 'backend',
      code: commandError.code,
      cause: commandError,
    });
    expect(vi.mocked(invoke).mock.calls[0]?.[1]).toMatchObject({ demand });
  });

  it('rejects a malformed demand before invoking execute_graph_document', async () => {
    const malformed = { type: 'default', extra: true } as unknown as ExecutionDemandDto;

    await expect(ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      malformed,
    )).rejects.toThrow('Invalid default execution demand');

    expect(invoke).not.toHaveBeenCalled();
  });


  it('invokes execute_graph_document and drains its RunEvent channel before resolving', async () => {
    let resolveInvoke!: () => void;
    vi.mocked(invoke).mockReturnValue(new Promise<void>((resolve) => {
      resolveInvoke = resolve;
    }));
    const received: RunEvent[] = [];

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      (event) => received.push(event),
    );

    expect(invoke).toHaveBeenCalledOnce();
    const [command, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; demand: { type: 'default' }; onEvent: Channel<RunEvent> },
    ];
    expect(command).toBe('execute_graph_document');
    expect(args).toEqual({
      projectInstanceId,
      graphPath: 'events/Main.yssbi-event',
      demand: { type: 'default' },
      onEvent: expect.any(Channel),
    });

    resolveInvoke();
    let settled = false;
    void execution.then(() => {
      settled = true;
    });
    await Promise.resolve();
    expect(settled).toBe(false);

    const completed = runEvent({ type: 'runCompleted' });
    args.onEvent.onmessage?.(completed);

    await expect(execution).resolves.toBeUndefined();
    expect(received).toEqual([completed]);
  });

  it('routes ordered user output through a callback separate from RunEvent consumers', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const receivedRunEvents: RunEvent[] = [];
    const receivedOutput: RunOutputChannelEvent[] = [];
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      (event) => receivedRunEvents.push(event),
      (event) => receivedOutput.push(event),
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { onEvent: Channel<unknown> },
    ];
    const output: RunOutputChannelEvent = {
      runId: '41',
      sequence: 1,
      stream: 'stdout',
      text: 'user-visible value',
      sourceGraphPath: 'functions/output.yssbi-function',
      sourceNodeId: '00000000-0000-0000-0000-000000000002',
      sourcePort: {
        kind: 'declared',
        nodeId: '00000000-0000-0000-0000-000000000002',
        portKey: 'message',
      },
    };
    const status: RunOutputChannelEvent = {
      runId: '41',
      sequence: 2,
      stream: 'stdout',
      status: 'truncated',
      sourceGraphPath: 'functions/output.yssbi-function',
      sourceNodeId: '00000000-0000-0000-0000-000000000002',
      sourcePort: {
        kind: 'declared',
        nodeId: '00000000-0000-0000-0000-000000000002',
        portKey: 'message',
      },
    };
    const completed = runEvent({ type: 'runCompleted' });

    args.onEvent.onmessage?.(output);
    args.onEvent.onmessage?.(status);
    args.onEvent.onmessage?.(completed);

    await expect(execution).resolves.toBeUndefined();
    expect(receivedOutput).toEqual([output, status]);
    expect(receivedRunEvents).toEqual([completed]);
  });

  it('surfaces a throwing runCompleted consumer after a successful invoke settles', async () => {
    const consumerError = new Error('runCompleted consumer failed');
    vi.mocked(invoke).mockResolvedValue(undefined);
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      () => { throw consumerError; },
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];

    expect(() => args.onEvent.onmessage?.(runEvent({ type: 'runCompleted' }))).not.toThrow();

    await expect(execution).rejects.toBe(consumerError);
  });

  it.each([
    {
      terminal: { type: 'runErrored', code: 'kernelFailed', phase: null } as const,
      commandError: {
        code: 'run_failed',
        details: { terminalRunEventSent: true },
        incidentId: null,
      },
    },
    {
      terminal: { type: 'runCancelled' } as const,
      commandError: {
        code: 'run_cancelled',
        details: { terminalRunEventSent: true },
        incidentId: null,
      },
    },
  ])('preserves the backend rejection when the $terminal.type consumer throws', async ({
    terminal,
    commandError,
  }) => {
    const consumerError = new Error(`${terminal.type} consumer failed`);
    vi.mocked(invoke).mockRejectedValue(commandError);
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      () => { throw consumerError; },
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];

    expect(() => args.onEvent.onmessage?.(runEvent(terminal))).not.toThrow();

    await expect(execution).rejects.toMatchObject({
      kind: 'backend',
      code: commandError.code,
      details: commandError.details,
      cause: commandError,
    });
  });

  it('rejects its pending drain when HMR disposes the execution channel', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];
    const outcome = execution.then(
      () => 'resolved' as const,
      (error: unknown) => error,
    );

    disposeTrackedChannelsForHmr();
    const observed = await Promise.race([
      outcome,
      new Promise<'timeout'>((resolve) => setTimeout(() => resolve('timeout'), 10)),
    ]);
    if (observed === 'timeout') {
      args.onEvent.onmessage?.(runEvent({ type: 'runCompleted' }));
      await execution;
    }

    expect(observed).toMatchObject({ code: 'execution_channel_disposed' });
  });

  it('preserves the command error when HMR disposes its pending terminal drain', async () => {
    const commandError = {
      code: 'run_cancelled',
      details: { terminalRunEventSent: true },
      incidentId: null,
    };
    vi.mocked(invoke).mockRejectedValue(commandError);

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
    );
    disposeTrackedChannelsForHmr();

    await expect(execution).rejects.toMatchObject({
      kind: 'backend',
      code: commandError.code,
      cause: commandError,
    });
  });

  it('drains a terminal run event before rethrowing the original command error', async () => {
    const commandError = {
      code: 'run_failed',
      details: { terminalRunEventSent: true },
      incidentId: null,
    };
    vi.mocked(invoke).mockRejectedValue(commandError);
    const received: RunEvent[] = [];

    const execution = ProjectService.executeGraphDocument(
      projectInstanceId,
      'events/Main.yssbi-event',
      { type: 'default' },
      (event) => received.push(event),
    );
    const [, args] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { graphPath: string; onEvent: Channel<RunEvent> },
    ];

    let settled = false;
    void execution.catch(() => {
      settled = true;
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(settled).toBe(false);

    const errored = runEvent({ type: 'runErrored', code: 'kernelFailed', phase: null });
    args.onEvent.onmessage?.(errored);

    await expect(execution).rejects.toMatchObject({
      kind: 'backend',
      code: commandError.code,
      cause: commandError,
    });
    expect(received).toEqual([errored]);
  });
});
